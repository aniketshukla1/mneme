//! # mneme-evolve
//!
//! Memory evolution — the *supporting* mechanism. When a memory is written,
//! a bounded, async worker retroactively re-tags / re-links semantically
//! related memories (A-MEM style, arXiv:2502.12110), but with the safety
//! bounds the report §3 insists on:
//!
//! - never overwrite — invalidate + create a new bi-temporal version
//! - `MAX_EVOLVE_PER_WRITE` hard cap on cascade
//! - per-memory cooldown + lifetime evolution limit
//! - scope/tenant filter before any evolution fires
//!
//! STATUS: scaffold. Phase 1. See CLAUDE.md.

/// Tunable bounds for the evolution worker (report §3, "hard caps").
#[derive(Debug, Clone)]
pub struct EvolveConfig {
    pub max_evolve_per_write: usize,
    pub max_lifetime_evolutions: u16,
    pub cooldown_secs: u64,
    /// Minimum structural change for an evolution to be persisted at all.
    pub min_change_threshold: f32,
}

impl Default for EvolveConfig {
    fn default() -> Self {
        Self {
            max_evolve_per_write: 3,
            max_lifetime_evolutions: 8,
            cooldown_secs: 300,
            min_change_threshold: 0.15,
        }
    }
}

/// The async worker. Consumes `MemoryWritten` events; emits `MemoryEvolved`.
pub struct EvolutionWorker {
    pub config: EvolveConfig,
}

impl EvolutionWorker {
    pub fn new(config: EvolveConfig) -> Self {
        Self { config }
    }
    // TODO(phase-1): note construction (P_s1), link generation (P_s2),
    // bounded evolution (P_s3) — all off the write path.
}
