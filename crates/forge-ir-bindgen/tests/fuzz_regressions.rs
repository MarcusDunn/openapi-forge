//! Replay every committed `transformer_output_ir` crash sample and assert
//! the host-side IR-consumption code does not panic. Stable toolchain;
//! no nightly required.
//!
//! When `cargo +nightly fuzz` finds a crash in `transformer_output_ir`,
//! minimize it (`cargo fuzz tmin transformer_output_ir <crash>`) and copy
//! the minimized input into `tests/fuzz_regressions/` with a descriptive
//! name (e.g. `dangling-discriminator.json`). After fixing the bug, this
//! test prevents the same input from regressing.
//!
//! The replay mirrors the fuzz target: parse as `forge_ir::Ir`, run
//! `validate_refs`, then both world round-trips. Returning a
//! `BindgenError` is fine; panicking is the bug class we guard against.

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
        let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if name.starts_with('.') {
            continue;
        }

        let bytes = fs::read(&path).expect("read fuzz regression");
        let Ok(s) = std::str::from_utf8(&bytes) else {
            continue;
        };
        let Ok(ir) = serde_json::from_str::<forge_ir::Ir>(s) else {
            continue;
        };

        let _ = forge_ir_bindgen::validate_refs(&ir);

        let wit_t = forge_ir_bindgen::convert::transformer::ir_to_wit(ir.clone());
        let _ = forge_ir_bindgen::convert::transformer::ir_from_wit(wit_t);

        let wit_g = forge_ir_bindgen::convert::generator::ir_to_wit(ir);
        let _ = forge_ir_bindgen::convert::generator::ir_from_wit(wit_g);

        replayed += 1;
    }

    eprintln!(
        "replayed {replayed} fuzz regression sample(s) from {}",
        dir.display()
    );
}
