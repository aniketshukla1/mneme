//! # mneme-core
//!
//! Core types and traits for **mneme** — a self-improving long-term memory
//! layer for AI agents.
//!
//! The wedge: a *procedural-memory compiler* that turns agent trajectories
//! plus outcomes into improved, versioned [`entity::PolicyArtifact`]s, gated
//! by mandatory shadow-evaluation. *Memory evolution* (A-MEM-style retroactive
//! re-linking) is the supporting substrate.
//!
//! This crate has no I/O. It defines the shared vocabulary every other crate
//! in the workspace builds on. Start reading at [`event::Event`] (the system
//! of record) and [`traits`] (the seams).

pub mod entity;
pub mod event;
pub mod traits;
pub mod types;

pub use entity::{ArtifactKind, Canary, Memory, Outcome, PolicyArtifact, Provenance};
pub use event::{ChangeSet, EvalReport, Event, LogEntry};
pub use traits::{
    Embedder, EventLog, Hit, LlmClient, MaterializedView, MnemeError, Query, Retriever,
};
pub use types::{new_id, BiTemporal, Id, Scope};
