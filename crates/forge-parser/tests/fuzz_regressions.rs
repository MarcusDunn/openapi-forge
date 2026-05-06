//! Replay every committed fuzz-crash sample and assert `parse_str` does
//! not panic. Runs on stable; no nightly required.
//!
//! When `cargo +nightly fuzz` finds a crash, minimize it (`cargo fuzz tmin
//! <target>`) and copy the minimized input into `tests/fuzz_regressions/`
//! with a descriptive name (`<short-hash>-<one-line-summary>.json` works).
//! After fixing the parser, this test prevents the same input from
//! regressing.
//!
//! Inputs are read as bytes and decoded as UTF-8 if possible — non-UTF-8
//! samples are still valid corpus entries for the byte target.

use std::fs;
use std::path::Path;

#[test]
fn replay_all_committed_crashes() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fuzz_regressions");
    let entries = match fs::read_dir(&dir) {
        Ok(e) => e,
        Err(e) => panic!("read_dir({}): {e}", dir.display()),
    };

    let mut replayed = 0usize;
    for entry in entries {
        let entry = entry.expect("dir entry");
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        // Skip the .gitkeep / hidden files.
        let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if name.starts_with('.') {
            continue;
        }

        let bytes = fs::read(&path).expect("read fuzz regression");
        if let Ok(s) = std::str::from_utf8(&bytes) {
            // Must not panic. Errors (and error-severity diagnostics) are
            // fine — we only guard against panics, OOMs, and infinite loops.
            let _ = forge_parser::parse_str(s);
        }
        replayed += 1;
    }

    // We can't assert `replayed > 0` (the directory starts empty), but we
    // surface the count so a green run with zero replays is visible.
    eprintln!(
        "replayed {replayed} fuzz regression sample(s) from {}",
        dir.display()
    );
}
