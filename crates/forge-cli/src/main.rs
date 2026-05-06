//! `forge` CLI.
//!
//! Two configuration modes:
//!
//! - **Project mode** — read `forge.toml` from the project directory.
//!   Supports per-plugin config blocks and is the recommended layout
//!   for repeatable runs.
//! - **Config-less mode** — pass `--input`, `--transformer`,
//!   `--generator`, and `--out` directly on the command line. Useful
//!   for one-off runs and shell scripting; per-plugin config defaults
//!   to `{}`. Triggered when `--input` is set.
//!
//! Two input forms (project mode supports both via `forge.toml`;
//! config-less mode supports spec only):
//!
//! - spec — parse an OpenAPI 3.0 JSON document through `forge-parser`.
//! - ir — load a canonical `forge_ir::Ir` directly (debugging escape
//!   hatch; bypasses the parser). Only reachable via
//!   `[input] ir = "..."` in `forge.toml`.

use std::path::{Path, PathBuf};

use clap::{Parser, Subcommand};
use forge_host::{Engine, Plugin};
use forge_pipeline::{run as run_pipeline, PipelineConfig};
use serde::Deserialize;

mod oci;

#[derive(Debug, Parser)]
#[command(name = "forge", version, about = "OpenAPI Forge")]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Debug, Subcommand)]
enum Cmd {
    /// Run the configured pipeline.
    ///
    /// Without `--input`, reads `forge.toml` from `<project>` (project
    /// mode). With `--input`, builds the config from CLI flags and
    /// ignores `forge.toml` (config-less mode). The pre-parsed IR input
    /// form is reachable via `[input] ir = "..."` in `forge.toml`.
    Generate {
        /// Project directory containing `forge.toml`. Used as the path
        /// resolution base in project mode. Ignored in config-less mode.
        #[arg(default_value = ".")]
        project: PathBuf,
        /// OpenAPI 3.0 JSON spec. Triggers config-less mode.
        #[arg(short = 'i', long = "input", value_name = "SPEC")]
        input: Option<PathBuf>,
        /// Transformer plugin (`.wasm` path or OCI ref). Repeat to chain.
        /// Config-less mode only.
        #[arg(long = "transformer", value_name = "REF")]
        transformer: Vec<String>,
        /// Generator plugin (`.wasm` path or OCI ref). Required in
        /// config-less mode.
        #[arg(long = "generator", value_name = "REF")]
        generator: Option<String>,
        /// Output directory. Required in config-less mode; overrides
        /// `[output] dir` from `forge.toml` in project mode.
        #[arg(short = 'o', long = "out")]
        out: Option<PathBuf>,
    },
    /// Print the version of the IR contract this CLI was built against.
    IrVersion,
}

#[derive(Debug, Deserialize)]
struct Project {
    input: Input,
    #[serde(default)]
    transformers: Vec<PluginRef>,
    generator: PluginRef,
    output: Output,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum Input {
    /// Parse an OpenAPI 3.0 JSON document through `forge-parser`.
    Spec { spec: PathBuf },
    /// Load a canonical `forge_ir::Ir` JSON directly. Bypasses the parser
    /// — useful when iterating on transformers/generators without a real
    /// spec.
    Ir { ir: PathBuf },
}

#[derive(Debug, Deserialize)]
struct PluginRef {
    #[serde(flatten)]
    source: PluginSource,
    /// Plugin-specific JSON-shaped config. Serialized to JSON and handed
    /// to the plugin as the `config` argument.
    #[serde(default = "empty_config")]
    config: serde_json::Value,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum PluginSource {
    /// `.wasm` component path, relative to the project directory.
    Wasm { wasm: PathBuf },
    /// OCI reference, e.g. `ghcr.io/org/plugin:1.0.0` or
    /// `registry.example/repo@sha256:...`. Pulled lazily on
    /// `forge generate` and cached under the user's XDG cache dir.
    Oci { oci: String },
}

fn empty_config() -> serde_json::Value {
    serde_json::json!({})
}

#[derive(Debug, Deserialize)]
struct Output {
    dir: PathBuf,
}

#[derive(Debug, thiserror::Error)]
enum CliError {
    #[error("failed to read {path}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to parse forge.toml: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("failed to parse {path} as IR JSON: {source}")]
    BadIr {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("parser failed: {0}")]
    Parse(#[from] forge_parser::ParseError),
    #[error("parser produced {count} error-severity diagnostic(s); halting")]
    ParseDiagnostics { count: usize },
    #[error("engine init: {0}")]
    Engine(String),
    #[error("plugin load ({origin}): {reason}")]
    PluginLoad { origin: String, reason: String },
    #[error("oci pull ({reference}): {source}")]
    Oci {
        reference: String,
        #[source]
        source: oci::OciError,
    },
    #[error("plugin {plugin}: invalid config_schema(): {reason}")]
    ConfigSchemaInvalid { plugin: String, reason: String },
    #[error("plugin {plugin}: config validation failed:\n  - {}", errors.join("\n  - "))]
    ConfigValidation { plugin: String, errors: Vec<String> },
    #[error("pipeline: {0}")]
    Pipeline(#[from] forge_pipeline::PipelineError),
    #[error("output guard: {0}")]
    Output(#[from] forge_host::filesystem::OutputError),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("--generator is required in config-less mode (when --input is set)")]
    MissingGenerator,
    #[error("--out is required in config-less mode (when --input is set)")]
    MissingOut,
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_writer(std::io::stderr)
        .init();

    let cli = Cli::parse();
    let result = match cli.command {
        Cmd::Generate {
            project,
            input,
            transformer,
            generator,
            out,
        } => match input {
            Some(spec) => {
                generate_from_args(spec, &transformer, generator.as_deref(), out.as_deref())
            }
            None => generate_from_project(&project, out.as_deref()),
        },
        Cmd::IrVersion => {
            println!("{}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
    };
    if let Err(e) = result {
        eprintln!("error: {e}");
        let mut src = std::error::Error::source(&e);
        while let Some(err) = src {
            eprintln!("  caused by: {err}");
            src = err.source();
        }
        std::process::exit(2);
    }
}

/// Project mode: read `forge.toml` from `project`, then run.
fn generate_from_project(project: &Path, out_override: Option<&Path>) -> Result<(), CliError> {
    let manifest_path = project.join("forge.toml");
    let manifest_str = std::fs::read_to_string(&manifest_path).map_err(|e| CliError::Read {
        path: manifest_path.clone(),
        source: e,
    })?;
    let cfg: Project = toml::from_str(&manifest_str)?;
    run_generate(project, &cfg, out_override)
}

/// Config-less mode: build a `Project` from CLI flags. Plugin paths
/// are resolved relative to the current working directory.
fn generate_from_args(
    spec: PathBuf,
    transformer: &[String],
    generator: Option<&str>,
    out_override: Option<&Path>,
) -> Result<(), CliError> {
    let generator = generator.ok_or(CliError::MissingGenerator)?;
    let out_dir = out_override.ok_or(CliError::MissingOut)?.to_path_buf();

    let cfg = Project {
        input: Input::Spec { spec },
        transformers: transformer
            .iter()
            .map(|s| PluginRef {
                source: parse_plugin_arg(s),
                config: empty_config(),
            })
            .collect(),
        generator: PluginRef {
            source: parse_plugin_arg(generator),
            config: empty_config(),
        },
        output: Output {
            dir: out_dir.clone(),
        },
    };

    // In config-less mode, paths in CLI args are relative to CWD; pass
    // `.` as the resolution base. The output dir is already absolute or
    // CWD-relative, so we forward it as the override too.
    run_generate(Path::new("."), &cfg, Some(&out_dir))
}

/// `s` is either a `.wasm` path on disk or an OCI reference. Heuristic:
/// if it ends in `.wasm` or names an existing file, treat as a path;
/// otherwise treat as an OCI ref. The OCI puller will surface a parse
/// error if the string is neither.
fn parse_plugin_arg(s: &str) -> PluginSource {
    let path = Path::new(s);
    let looks_like_wasm = s.ends_with(".wasm") || path.is_file();
    if looks_like_wasm {
        PluginSource::Wasm {
            wasm: path.to_path_buf(),
        }
    } else {
        PluginSource::Oci { oci: s.to_owned() }
    }
}

fn run_generate(
    project: &Path,
    cfg: &Project,
    out_override: Option<&Path>,
) -> Result<(), CliError> {
    let ir = load_ir(project, &cfg.input)?;

    let engine = Engine::new().map_err(|e| CliError::Engine(e.to_string()))?;

    let mut transformers: Vec<Plugin> = Vec::with_capacity(cfg.transformers.len());
    let mut configs: Vec<String> = Vec::with_capacity(cfg.transformers.len() + 1);
    for t in &cfg.transformers {
        let (bytes, label) = load_plugin_bytes(project, &t.source)?;
        let plugin =
            Plugin::load_transformer(&engine, &bytes).map_err(|e| CliError::PluginLoad {
                origin: label.clone(),
                reason: e.to_string(),
            })?;
        validate_config(plugin.config_schema(), &t.config, &label)?;
        transformers.push(plugin);
        configs.push(t.config.to_string());
    }

    let (gen_bytes, gen_label) = load_plugin_bytes(project, &cfg.generator.source)?;
    let generator =
        Plugin::load_generator(&engine, &gen_bytes).map_err(|e| CliError::PluginLoad {
            origin: gen_label.clone(),
            reason: e.to_string(),
        })?;
    validate_config(generator.config_schema(), &cfg.generator.config, &gen_label)?;
    configs.push(cfg.generator.config.to_string());

    let pipe_cfg = PipelineConfig {
        configs,
        ..Default::default()
    };
    let xforms: Vec<&Plugin> = transformers.iter().collect();
    let out = run_pipeline(&engine, ir, &xforms, &generator, &pipe_cfg)?;

    // Validate output before writing. Use the generator's limits to seed
    // the caps; this matches what the host enforced inside the WASM call.
    let caps = forge_host::filesystem::Caps::from_limits(forge_host::Limits::generator());
    forge_host::filesystem::validate_output(&out.generation.files, caps)?;

    let out_dir = out_override
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| project.join(&cfg.output.dir));
    std::fs::create_dir_all(&out_dir)?;
    for f in &out.generation.files {
        let target = out_dir.join(&f.path);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&target, &f.content)?;
    }

    println!(
        "wrote {} file(s) to {} ({} diagnostic(s))",
        out.generation.files.len(),
        out_dir.display(),
        out.diagnostics.len(),
    );
    Ok(())
}

/// Load the IR for the configured `[input]`, branching on whether the
/// project asks for a parsed spec or raw IR JSON.
fn load_ir(project: &Path, input: &Input) -> Result<forge_ir::Ir, CliError> {
    match input {
        Input::Spec { spec } => {
            let spec_path = project.join(spec);
            // `parse_path` enables the file-based $ref resolver so specs
            // that split components / paths across sibling files (the
            // common multi-document layout) work. The in-memory
            // `parse_str_with_file` rejects external $refs outright;
            // using it here would regress split-document support shipped
            // in #56.
            let out = forge_parser::parse_path(&spec_path)?;
            print_diagnostics(&out.diagnostics);
            let errs = out
                .diagnostics
                .iter()
                .filter(|d| d.severity == forge_ir::Severity::Error)
                .count();
            if errs > 0 {
                return Err(CliError::ParseDiagnostics { count: errs });
            }
            out.spec
                .ok_or(CliError::ParseDiagnostics { count: errs.max(1) })
        }
        Input::Ir { ir } => {
            let ir_path = project.join(ir);
            let ir_str = std::fs::read_to_string(&ir_path).map_err(|e| CliError::Read {
                path: ir_path.clone(),
                source: e,
            })?;
            serde_json::from_str(&ir_str).map_err(|e| CliError::BadIr {
                path: ir_path,
                source: e,
            })
        }
    }
}

/// Friendly label for diagnostics — the wasm filename, falling back to the
/// full path if it has no filename component.
fn plugin_label(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

/// Resolve a `PluginSource` to raw component bytes plus a human label
/// for diagnostics. Filesystem refs read from `project`-relative paths;
/// OCI refs are pulled (or served from cache) by `oci::fetch_to_bytes`.
fn load_plugin_bytes(project: &Path, source: &PluginSource) -> Result<(Vec<u8>, String), CliError> {
    match source {
        PluginSource::Wasm { wasm } => {
            let path = project.join(wasm);
            let bytes = std::fs::read(&path).map_err(|e| CliError::Read {
                path: path.clone(),
                source: e,
            })?;
            Ok((bytes, plugin_label(&path)))
        }
        PluginSource::Oci { oci } => {
            let bytes = oci::fetch_to_bytes(oci).map_err(|source| CliError::Oci {
                reference: oci.clone(),
                source,
            })?;
            Ok((bytes, oci.clone()))
        }
    }
}

/// Validate `config` against the JSON Schema returned by a plugin's
/// `config_schema()` export. The schema is parsed once per plugin load; on
/// validation failure we surface every error so the user can fix them in one
/// pass instead of re-running for each.
fn validate_config(
    schema_str: &str,
    config: &serde_json::Value,
    plugin_label: &str,
) -> Result<(), CliError> {
    let schema_value: serde_json::Value =
        serde_json::from_str(schema_str).map_err(|e| CliError::ConfigSchemaInvalid {
            plugin: plugin_label.to_owned(),
            reason: e.to_string(),
        })?;
    let validator =
        jsonschema::validator_for(&schema_value).map_err(|e| CliError::ConfigSchemaInvalid {
            plugin: plugin_label.to_owned(),
            reason: e.to_string(),
        })?;
    let errors: Vec<String> = validator
        .iter_errors(config)
        .map(|e| format!("{} (at {})", e, e.instance_path))
        .collect();
    if !errors.is_empty() {
        return Err(CliError::ConfigValidation {
            plugin: plugin_label.to_owned(),
            errors,
        });
    }
    Ok(())
}

fn print_diagnostics(diagnostics: &[forge_ir::Diagnostic]) {
    use forge_ir::Severity;
    for d in diagnostics {
        let label = match d.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Info => "info",
            Severity::Hint => "hint",
        };
        let location = d
            .location
            .as_ref()
            .map(|l| {
                if let Some(file) = &l.file {
                    format!("{}#{}", file, l.pointer)
                } else {
                    l.pointer.clone()
                }
            })
            .unwrap_or_default();
        eprintln!("{label} [{}] {} ({})", d.code, d.message, location);
    }
}
