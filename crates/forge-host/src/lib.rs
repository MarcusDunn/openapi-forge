//! WASM plugin runtime.
//!
//! Loads sandboxed WASM components, enforces fuel / memory / epoch limits,
//! provides the `host-api` imports declared in `wit/host.wit`, and invokes
//! plugin exports.
//!
//! # Sandbox
//!
//! Plugins receive **no** filesystem, network, clock, RNG, environment, or
//! process state. The only host imports are `log` and `case-convert`. WASI
//! imports are not linked. See ADR-0001 and ADR-0008.
//!
//! # Resource limits
//!
//! Every plugin invocation has its own `Store` / `StoreLimits` / `wasmtime`
//! resource budget:
//!
//! * **Fuel** — instructions executed. `consume_fuel = true` on the engine.
//! * **Memory** — bytes allocated by guest linear memory. Enforced via
//!   `wasmtime::ResourceLimiter`.
//! * **Wall-clock time** — enforced via epoch interruption. The engine has a
//!   shared epoch counter ticked by a background thread on a fixed cadence;
//!   each store sets `epoch_deadline_trap()` at the appropriate count.

#![forbid(unsafe_code)]

pub mod filesystem;
mod runtime;

pub use runtime::{
    Engine, EngineError, FileMode, GenerationOutput, HostState, Limits, LoadError, OutputFile,
    Plugin, PluginKind, ResourceKind, StageError, TransformOutput,
};
