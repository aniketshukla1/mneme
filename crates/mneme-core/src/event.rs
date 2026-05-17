//! The append-only event log is the single system of record. Every index
//! (vector, BM25, graph, procedural) is a materialized view rebuildable by
//! replaying these events. See architecture report §4 and §8.

use crate::entity::{Memory, Outcome, PolicyArtifact};
use crate::types::*;
use serde::{Deserialize, Serialize};

/// One immutable entry in the log.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub id: Id,
    pub event: Event,
}

/// Everything that can happen in the system. Memory writes and outcome
/// records are commutative; procedural commits are NOT (single-writer).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Event {
    MemoryWritten(Memory),
    MemoryEvolved { from: MemoryRef, to: MemoryRef, diff: ChangeSet },
    MemoryInvalidated { id: MemoryRef, reason: String },

    OutcomeRecorded(Outcome),

    ProceduralProposed { proposal: ProposalId, artifacts: Vec<PolicyArtifact> },
    ProceduralCommitted { proposal: ProposalId, report: EvalReport },
    ProceduralRejected { proposal: ProposalId, reason: String },
}

/// A structural diff produced by the evolution worker.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeSet {
    pub keywords_added: Vec<String>,
    pub keywords_removed: Vec<String>,
    pub tags_added: Vec<String>,
    pub tags_removed: Vec<String>,
    pub context_rewritten: bool,
}

/// The outcome of shadow-evaluating a procedural proposal before commit.
/// This is the regression guard LangMem omits (report §2, §10).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalReport {
    pub canaries_passed: u32,
    pub canaries_total: u32,
    pub replay_success_rate: f32,
    pub safety_probe_passed: bool,
    pub objective_delta: f32,
}

impl EvalReport {
    /// A proposal may commit only if it clears every gate.
    pub fn is_committable(&self) -> bool {
        self.canaries_passed == self.canaries_total
            && self.safety_probe_passed
            && self.objective_delta >= 0.0
    }
}
