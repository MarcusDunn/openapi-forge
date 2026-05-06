//! Stage orchestration: parse → light-normalize → transformers →
//! full-normalize → generator → output guard.
//!
//! The driver collects **all** diagnostics from a stage before deciding
//! whether to halt. The default policy halts before the next stage if the
//! preceding stage produced any `error`-severity diagnostics.

#![forbid(unsafe_code)]

mod driver;

pub use driver::{run, PipelineConfig, PipelineError, PipelineOutput, StagePolicy};
