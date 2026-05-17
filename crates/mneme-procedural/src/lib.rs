//! # mneme-procedural
//!
//! The procedural-memory compiler — **the wedge**. Turns batches of
//! `Outcome`s into improved, versioned `PolicyArtifact`s via a GEPA-style
//! (arXiv:2507.19457) reflective loop: reflect -> propose K candidates ->
//! shadow-evaluate -> Pareto-select -> gated commit.
//!
//! The non-negotiable invariant (report §2, §10): NOTHING commits without
//! passing `EvalReport::is_committable()` — canaries + safety probe +
//! non-negative objective delta. This is the regression guard LangMem omits.
//!
//! STATUS: scaffold. Phase 2 — do not start until Phase 0 + 1 are green.

use mneme_core::{Id, MnemeError, Scope};

/// What triggers a compile pass: a count- or time-based outcome batch.
#[derive(Debug, Clone)]
pub struct CompileTrigger {
    pub min_batch: usize,
    pub max_age_secs: u64,
}

impl Default for CompileTrigger {
    fn default() -> Self {
        Self { min_batch: 32, max_age_secs: 3600 }
    }
}

/// The compiler. Holds an advisory lock on artifacts it is mutating; reads
/// continue against the current active version (atomic hot-swap).
pub struct ProceduralCompiler {
    pub trigger: CompileTrigger,
    /// Number of candidate revisions to generate per artifact.
    pub candidates_per_artifact: usize,
}

impl ProceduralCompiler {
    pub fn new() -> Self {
        Self { trigger: CompileTrigger::default(), candidates_per_artifact: 4 }
    }

    /// Phase 2 entry point. Reflect -> propose -> shadow-eval -> commit-or-reject.
    pub async fn compile(&self, _scope: &Scope) -> Result<Option<Id>, MnemeError> {
        // TODO(phase-2): GEPA loop. MUST gate on EvalReport::is_committable().
        Ok(None)
    }
}

impl Default for ProceduralCompiler {
    fn default() -> Self {
        Self::new()
    }
}
