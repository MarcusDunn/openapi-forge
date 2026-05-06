//! Build automation. Run via `cargo xtask <command>`.

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "xtask")]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Debug, Subcommand)]
enum Cmd {
    /// Run everything that CI runs, in dependency order.
    Ci,
    /// Run formatter check.
    Fmt,
    /// Run clippy with `-D warnings`.
    Clippy,
    /// Run unit + integration tests via nextest if available, else `cargo test`.
    Test,
    /// Run `cargo doc --no-deps`.
    Doc,
    /// Build the plugins workspace for `wasm32-wasip2`. Required before
    /// running the host integration tests that load real plugins.
    Plugins,
    /// Scan `plugins/*/src/` for forbidden native-test attributes
    /// (`#[test]`, `#[cfg(test)]`, etc.). All plugin tests must live in
    /// `crates/forge-plugin-itests/` and route through
    /// `forge-test-harness::PluginRunner`. See ADR-0004.
    PluginTestDiscipline,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let result = match cli.command {
        Cmd::Ci => run_ci(),
        Cmd::Fmt => run(&["fmt", "--all", "--", "--check"]),
        Cmd::Clippy => run(&[
            "clippy",
            "--workspace",
            "--all-targets",
            "--",
            "-D",
            "warnings",
        ]),
        Cmd::Test => run_test(),
        Cmd::Doc => run(&["doc", "--workspace", "--no-deps"]),
        Cmd::Plugins => build_plugins(),
        Cmd::PluginTestDiscipline => plugin_test_discipline(),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(code) => ExitCode::from(code),
    }
}

fn run_ci() -> Result<(), u8> {
    run(&["fmt", "--all", "--", "--check"])?;
    run(&[
        "clippy",
        "--workspace",
        "--all-targets",
        "--",
        "-D",
        "warnings",
    ])?;
    plugin_test_discipline()?;
    // Plugins must build before host integration tests run.
    build_plugins()?;
    run_test()?;
    run(&["doc", "--workspace", "--no-deps"])?;
    Ok(())
}

fn run_test() -> Result<(), u8> {
    if which("cargo-nextest") {
        run(&["nextest", "run", "--workspace"])
    } else {
        run(&["test", "--workspace"])
    }
}

fn build_plugins() -> Result<(), u8> {
    // Pass `--target` explicitly: `plugins/.cargo/config.toml` only applies
    // when cargo is invoked from inside that workspace, but we're using
    // `--manifest-path` from the host workspace.
    run(&[
        "build",
        "--release",
        "--target",
        "wasm32-wasip2",
        "--manifest-path",
        "plugins/Cargo.toml",
    ])
}

fn run(args: &[&str]) -> Result<(), u8> {
    let status = Command::new(cargo()).args(args).status().map_err(|_| 1u8)?;
    if status.success() {
        Ok(())
    } else {
        Err(status.code().unwrap_or(1).try_into().unwrap_or(1))
    }
}

fn cargo() -> std::ffi::OsString {
    std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into())
}

fn which(bin: &str) -> bool {
    let exe = if cfg!(windows) {
        format!("{bin}.exe")
    } else {
        bin.to_string()
    };
    std::env::var_os("PATH")
        .map(|paths| std::env::split_paths(&paths).any(|p| p.join(&exe).is_file()))
        .unwrap_or(false)
}

/// Forbidden attribute prefixes inside `plugins/*/src/`. Plugins are
/// `cdylib`-only and the SDK refuses to build for the host, so any
/// `#[test]` / `#[cfg(test)]` is dead code at best (today's
/// validator-required-operation-id pre-removal) and a misleading invitation
/// at worst. Tests live in `crates/forge-plugin-itests/`.
const FORBIDDEN_PATTERNS: &[&str] = &["#[test]", "#[cfg(test)", "#[cfg(all(test", "#[cfg(any(test"];

fn plugin_test_discipline() -> Result<(), u8> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let plugins_dir = manifest_dir
        .parent()
        .expect("xtask has a parent")
        .join("plugins");

    let mut violations: Vec<String> = Vec::new();
    for entry in plugins_dir.read_dir().map_err(|e| {
        eprintln!(
            "plugin-test-discipline: read {}: {e}",
            plugins_dir.display()
        );
        1u8
    })? {
        let entry = entry.map_err(|_| 1u8)?;
        if !entry.path().is_dir() {
            continue;
        }
        let src = entry.path().join("src");
        if !src.exists() {
            continue;
        }
        scan_dir(&src, &mut violations).map_err(|_| 1u8)?;
    }

    if violations.is_empty() {
        return Ok(());
    }

    eprintln!("plugin-test-discipline: forbidden attributes found in plugin source:");
    for v in &violations {
        eprintln!("  {v}");
    }
    eprintln!(
        "\nPlugin tests live in `crates/forge-plugin-itests/tests/<plugin>.rs` \
         and route through `forge_test_harness::PluginRunner`. See ADR-0004 \
         and `docs/plugin-authoring.md`."
    );
    Err(1)
}

fn scan_dir(dir: &Path, out: &mut Vec<String>) -> std::io::Result<()> {
    for entry in dir.read_dir()? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            scan_dir(&path, out)?;
            continue;
        }
        if path.extension().and_then(|s| s.to_str()) != Some("rs") {
            continue;
        }
        let contents = std::fs::read_to_string(&path)?;
        for (i, raw) in contents.lines().enumerate() {
            let line = raw.trim_start();
            // Skip line comments to avoid false positives on docstrings
            // that quote the forbidden attribute as an example.
            if line.starts_with("//") {
                continue;
            }
            for pat in FORBIDDEN_PATTERNS {
                if line.contains(pat) {
                    out.push(format!("{}:{}: `{}`", path.display(), i + 1, pat));
                }
            }
        }
    }
    Ok(())
}
