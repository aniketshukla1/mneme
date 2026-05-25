//! # mneme-llm
//!
//! `LlmClient` implementations. The trait itself lives in
//! [`mneme_core::traits::LlmClient`]; this crate owns the concrete
//! backends so heavyweight deps (HTTP, ONNX, future things) don't
//! leak into `mneme-core`.
//!
//! Two implementations ship today:
//!
//! - [`FakeLlmClient`] — deterministic, dependency-free, always
//!   compiled. The Phase-1 [evolution worker][mneme-evolve] tests and
//!   the eventual Phase-2 procedural compiler tests run against it so
//!   the whole workspace's test suite stays fast and offline.
//! - [`OllamaLlmClient`] — real local backend talking to an Ollama
//!   instance over HTTP, behind the `ollama` feature flag. Default-on
//!   for production builds; CI / lean builds can opt out via
//!   `--no-default-features`.

pub mod fake;

#[cfg(feature = "ollama")]
pub mod ollama;

pub use fake::FakeLlmClient;

#[cfg(feature = "ollama")]
pub use ollama::OllamaLlmClient;
