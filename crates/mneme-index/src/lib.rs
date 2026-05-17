//! # mneme-index
//!
//! Materialized retrieval views over the event log: a vector index
//! (`hnsw_rs`, Phase 0) and a BM25 index (`tantivy`, Phase 0), unified by a
//! hybrid [`Retriever`]. The custom filtered-HNSW variant (report §7, Phase 3)
//! lands here later — keep this trait-shaped so it can be swapped in.
//!
//! STATUS: scaffold. See CLAUDE.md "Build order" — implement `VectorView`
//! first, then `Bm25View`, then `HybridRetriever`.

use mneme_core::{Hit, MnemeError, Query, Retriever};

/// Hybrid retriever fusing vector + BM25 (+ graph-walk, later) with an
/// explainable score breakdown. Reciprocal-rank fusion to start.
pub struct HybridRetriever {
    // TODO(phase-0): vector: VectorView, bm25: Bm25View
}

impl HybridRetriever {
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for HybridRetriever {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Retriever for HybridRetriever {
    async fn search(&self, _query: &Query) -> Result<Vec<Hit>, MnemeError> {
        // TODO(phase-0): fan out to vector + bm25 views, fuse with RRF.
        Ok(Vec::new())
    }
}
