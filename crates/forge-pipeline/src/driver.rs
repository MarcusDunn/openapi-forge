//! Pipeline driver.
//!
//! Run a parsed IR through a chain of transformer plugins, then a single
//! generator plugin, collecting diagnostics and respecting the halt-on-error
//! policy.
//!
//! The driver does **not** call the parser: it accepts an already-parsed
//! [`forge_ir::Ir`]. This keeps `forge-pipeline` independent of the spec
//! parser, so it composes cleanly in tests and in the test harness.

use forge_host::{Engine, GenerationOutput, Limits, Plugin, StageError};
use forge_ir::{Diagnostic, Ir, Severity};

#[derive(Debug, Clone, Default)]
pub struct PipelineConfig {
    /// JSON config strings per stage. Indexed parallel to `transformers`,
    /// followed by the generator config at the end. Stages with no config
    /// receive the empty object `"{}"`.
    pub configs: Vec<String>,
    pub policy: StagePolicy,
}

/// Halt-on-error policy. Default halts the pipeline before the next stage
/// if the preceding stage produced any `error`-severity diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StagePolicy {
    #[default]
    HaltOnError,
    AllowErrors,
}

#[derive(Debug)]
pub struct PipelineOutput {
    pub generation: GenerationOutput,
    /// Aggregated diagnostics from every stage, in execution order.
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, thiserror::Error)]
pub enum PipelineError {
    #[error("transformer `{plugin}` failed: {source}")]
    Transformer {
        plugin: String,
        #[source]
        source: StageError,
    },
    #[error("generator `{plugin}` failed: {source}")]
    Generator {
        plugin: String,
        #[source]
        source: StageError,
    },
    #[error("stage `{plugin}` produced {count} error-severity diagnostics; halting")]
    StageErrors { plugin: String, count: usize },
}

/// Run the configured pipeline.
///
/// `transformers` are applied in order. The result is then fed to
/// `generator`. Diagnostics are aggregated and surfaced via
/// [`PipelineOutput::diagnostics`]; the generator's files come back via
/// [`PipelineOutput::generation`].
pub fn run(
    _engine: &Engine,
    spec: Ir,
    transformers: &[&Plugin],
    generator: &Plugin,
    cfg: &PipelineConfig,
) -> Result<PipelineOutput, PipelineError> {
    let mut diagnostics: Vec<Diagnostic> = Vec::new();
    let mut current = spec;

    let empty = "{}".to_string();
    let cfg_or_empty = |i: usize| cfg.configs.get(i).unwrap_or(&empty).clone();

    for (i, t) in transformers.iter().enumerate() {
        let stage_cfg = cfg_or_empty(i);
        let out = t
            .transform(current, &stage_cfg, Limits::transformer())
            .map_err(|e| PipelineError::Transformer {
                plugin: t.info().name.clone(),
                source: e,
            })?;
        let err_count = out
            .diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Error)
            .count();
        diagnostics.extend(out.diagnostics);
        if err_count > 0 && cfg.policy == StagePolicy::HaltOnError {
            return Err(PipelineError::StageErrors {
                plugin: t.info().name.clone(),
                count: err_count,
            });
        }
        current = out.spec;
    }

    let gen_cfg = cfg_or_empty(transformers.len());
    let gen_out = generator
        .generate(current, &gen_cfg, Limits::generator())
        .map_err(|e| PipelineError::Generator {
            plugin: generator.info().name.clone(),
            source: e,
        })?;
    let err_count = gen_out
        .diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .count();
    diagnostics.extend(gen_out.diagnostics.iter().cloned());
    if err_count > 0 && cfg.policy == StagePolicy::HaltOnError {
        return Err(PipelineError::StageErrors {
            plugin: generator.info().name.clone(),
            count: err_count,
        });
    }

    Ok(PipelineOutput {
        generation: gen_out,
        diagnostics,
    })
}
