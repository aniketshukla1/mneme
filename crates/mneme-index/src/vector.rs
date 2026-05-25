//! Vector materialized view backed by `hnsw_rs`.
//!
//! Consumes the event tail and maintains an in-memory HNSW index keyed by an
//! internal `usize` slot id that maps back to the original `MemoryRef`.
//! Search is k-nearest L2 with a post-filter on [`Scope`] and a tombstone
//! bitset for soft-deleted memories.
//!
//! Phase 0 limitations (documented for the later phases that fix them):
//! - Embeddings are `Box::leak`ed because `hnsw_rs` stores references with a
//!   lifetime tied to the index. On a fresh process we always replay from
//!   the event log, so this is per-process growth, not a leak across runs.
//!   Phase 3's custom filtered-HNSW lifts this restriction.
//! - Scope filtering is post-filter, not native. With selective filters this
//!   hurts recall; Phase 3 makes the filter ACORN-style and native.
//! - The index is in-memory only. Persistence is a later slice — Phase 0
//!   still guarantees rebuildability via log replay (hard rule #4).

use async_trait::async_trait;
use hnsw_rs::hnsw::Hnsw;
use hnsw_rs::prelude::DistL2;
use mneme_core::event::{Event, LogEntry};
use mneme_core::traits::MaterializedView;
use mneme_core::types::MemoryRef;
use mneme_core::{Hit, Id, MnemeError, Scope};
use std::collections::{HashMap, HashSet};
use std::sync::RwLock;

/// Per-slot metadata held outside the hnsw graph so we can post-filter on
/// scope and resolve neighbors back to a [`MemoryRef`] without a store
/// roundtrip.
struct Slot {
    memory: MemoryRef,
    scope: Scope,
}

pub struct VectorView {
    index: RwLock<Hnsw<'static, f32, DistL2>>,
    slots: RwLock<Vec<Slot>>,
    /// Most recently inserted slot for each memory id. Evolved or invalidated
    /// memories keep their slot in the hnsw graph (no removal in `hnsw_rs`)
    /// but are tombstoned at query time.
    by_memory: RwLock<HashMap<MemoryRef, usize>>,
    /// Memories that arrived via `MemoryWritten` without an embedding — we
    /// don't allocate a hnsw slot until the matching `MemoryEmbedded` event
    /// lands and we know the vector. Scope is held here so the eventual
    /// insert can post-filter without re-reading the log.
    pending: RwLock<HashMap<MemoryRef, Scope>>,
    tombstones: RwLock<HashSet<usize>>,
    dim: usize,
    /// The embedder this view was constructed against. `MemoryEmbedded`
    /// events carrying a different `model_id` are refused — they belong to
    /// a different vector space and silently mixing them would corrupt
    /// search quality.
    expected_model_id: String,
    last_checkpoint: RwLock<Option<Id>>,
}

impl VectorView {
    /// Construct an empty view over vectors of dimension `dim`, configured
    /// for the embedder identified by `model_id`.
    ///
    /// HNSW parameters are conservative Phase 0 defaults; they'll be revisited
    /// when the custom filtered-HNSW variant lands (Phase 3) and again when
    /// real workloads inform the tuning.
    pub fn new(dim: usize, model_id: impl Into<String>) -> Self {
        let hnsw = Hnsw::<'static, f32, DistL2>::new(16, 100_000, 16, 200, DistL2);
        Self {
            index: RwLock::new(hnsw),
            slots: RwLock::new(Vec::new()),
            by_memory: RwLock::new(HashMap::new()),
            pending: RwLock::new(HashMap::new()),
            tombstones: RwLock::new(HashSet::new()),
            dim,
            expected_model_id: model_id.into(),
            last_checkpoint: RwLock::new(None),
        }
    }

    /// Embedding dimension this view was constructed with. Inserts and
    /// queries with a different dimension are rejected.
    pub fn dim(&self) -> usize {
        self.dim
    }

    /// The embedder this view is bound to. The startup mismatch check in
    /// `mneme-server` reads this to refuse incompatible embedders.
    pub fn expected_model_id(&self) -> &str {
        &self.expected_model_id
    }

    fn insert_internal(
        &self,
        memory: MemoryRef,
        scope: Scope,
        embedding: &[f32],
    ) -> Result<(), MnemeError> {
        if embedding.len() != self.dim {
            return Err(MnemeError::Index(format!(
                "embedding dim {} does not match view dim {}",
                embedding.len(),
                self.dim
            )));
        }
        // If the same memory id appears twice (e.g. re-inserted as part of
        // an in-place fixup) tombstone the prior slot so it stops surfacing.
        if let Some(&prior) = self.by_memory.read().unwrap().get(&memory) {
            self.tombstones.write().unwrap().insert(prior);
        }
        let slot_id = {
            let mut slots = self.slots.write().unwrap();
            let id = slots.len();
            slots.push(Slot { memory, scope });
            id
        };
        // `hnsw_rs` stores references with the index's lifetime. We elect
        // 'static via `Box::leak`; see the module-level note on why this is
        // acceptable for Phase 0.
        let leaked: &'static [f32] = Box::leak(embedding.to_vec().into_boxed_slice());
        self.index.read().unwrap().insert((leaked, slot_id));
        self.by_memory.write().unwrap().insert(memory, slot_id);
        Ok(())
    }

    /// k-nearest neighbors, post-filtered by scope and tombstones.
    ///
    /// Returns [`Hit`]s with an explainable single-signal breakdown. The
    /// hybrid retriever fuses this with BM25 (next slice) into the final
    /// caller-facing score.
    pub fn search(&self, query: &[f32], k: usize, scope: &Scope) -> Result<Vec<Hit>, MnemeError> {
        if query.len() != self.dim {
            return Err(MnemeError::Index(format!(
                "query dim {} does not match view dim {}",
                query.len(),
                self.dim
            )));
        }
        if k == 0 {
            return Ok(Vec::new());
        }
        // Over-fetch then post-filter. The 4x multiplier is a Phase 0 guess;
        // selective scopes need more headroom, which Phase 3's native filter
        // makes irrelevant.
        let over_k = k.saturating_mul(4).max(16);
        let ef_search = over_k.max(50);
        let neighbours = self.index.read().unwrap().search(query, over_k, ef_search);
        let slots = self.slots.read().unwrap();
        let tombstones = self.tombstones.read().unwrap();
        let mut hits = Vec::with_capacity(k);
        for n in neighbours {
            if tombstones.contains(&n.d_id) {
                continue;
            }
            let slot = match slots.get(n.d_id) {
                Some(s) => s,
                None => continue,
            };
            if !scope.contains(&slot.scope) {
                continue;
            }
            // Convert L2 distance to a similarity (larger == better) so every
            // Retriever caller can treat scores uniformly.
            let score = 1.0 / (1.0 + n.distance);
            hits.push(Hit {
                memory: slot.memory,
                score,
                breakdown: vec![("vector".to_string(), score)],
            });
            if hits.len() == k {
                break;
            }
        }
        Ok(hits)
    }
}

#[async_trait]
impl MaterializedView for VectorView {
    fn name(&self) -> &str {
        "vector-view"
    }

    async fn apply(&self, entry: &LogEntry) -> Result<(), MnemeError> {
        match &entry.event {
            Event::MemoryWritten(mem) => {
                let memory_ref = MemoryRef(mem.id);
                if let Some(emb) = &mem.embedding {
                    // Synchronous path: caller (tests, or a future "sync
                    // embed" writer) already produced the vector. Insert
                    // directly and clear any stale pending entry.
                    self.insert_internal(memory_ref, mem.scope.clone(), emb)?;
                    self.pending.write().unwrap().remove(&memory_ref);
                } else {
                    // Asynchronous path: the embedding worker will follow up
                    // with a `MemoryEmbedded` event. Remember the scope so
                    // that insert can post-filter without re-reading the log.
                    self.pending
                        .write()
                        .unwrap()
                        .insert(memory_ref, mem.scope.clone());
                    tracing::trace!(
                        memory = %mem.id,
                        "vector-view: queued for async embedding"
                    );
                }
            }
            Event::MemoryEmbedded {
                id,
                embedding,
                model_id,
            } => {
                if model_id != &self.expected_model_id {
                    // Refusing here is defense-in-depth — the startup
                    // mismatch check in mneme-server is the primary guard,
                    // but a view that started clean and saw a rogue event
                    // mid-stream still won't mix vector spaces.
                    tracing::warn!(
                        memory = ?id,
                        event_model = %model_id,
                        view_model = %self.expected_model_id,
                        "vector-view: refusing MemoryEmbedded from a different embedder"
                    );
                } else {
                    let scope = self.pending.write().unwrap().remove(id);
                    match scope {
                        Some(scope) => {
                            self.insert_internal(*id, scope, embedding)?;
                        }
                        None => {
                            // MemoryEmbedded arrived without a prior pending
                            // MemoryWritten. Either the memory was already
                            // inserted via the sync path or the log was
                            // partially replayed — either way we ignore
                            // rather than risk a phantom slot.
                            tracing::warn!(
                                memory = ?id,
                                "vector-view: MemoryEmbedded with no pending MemoryWritten; ignoring"
                            );
                        }
                    }
                }
            }
            Event::MemoryEvolved { from, .. } => {
                // The new version arrives separately as a MemoryWritten with
                // `parent = from`. Tombstoning the parent's slot here keeps
                // the old vector from surfacing in the window between the
                // evolution event and the new MemoryWritten landing.
                if let Some(&slot_id) = self.by_memory.read().unwrap().get(from) {
                    self.tombstones.write().unwrap().insert(slot_id);
                }
            }
            Event::MemoryInvalidated { id, .. } => {
                if let Some(&slot_id) = self.by_memory.read().unwrap().get(id) {
                    self.tombstones.write().unwrap().insert(slot_id);
                }
                // Also drop any pending-but-never-embedded entry so we don't
                // accidentally re-surface an invalidated memory if the
                // embedding event lands late.
                self.pending.write().unwrap().remove(id);
            }
            _ => {}
        }
        *self.last_checkpoint.write().unwrap() = Some(entry.id);
        Ok(())
    }

    async fn checkpoint(&self) -> Result<Option<Id>, MnemeError> {
        Ok(*self.last_checkpoint.read().unwrap())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mneme_core::entity::{Memory, Provenance};
    use mneme_core::event::ChangeSet;
    use mneme_core::types::{new_id, BiTemporal};

    fn mem_with(content: &str, scope: Scope, embedding: Vec<f32>) -> Memory {
        Memory {
            id: new_id(),
            scope,
            content: content.into(),
            keywords: vec![],
            tags: vec![],
            context: String::new(),
            embedding: Some(embedding),
            links: vec![],
            parent: None,
            evolution_count: 0,
            time: BiTemporal::now(),
            provenance: Provenance::default(),
            source: None,
            position: None,
        }
    }

    fn entry(event: Event) -> LogEntry {
        LogEntry {
            id: new_id(),
            event,
        }
    }

    /// Push a ring of low-relevance filler vectors into a view so HNSW's
    /// small-graph randomness can't hide a real assertion failure. The
    /// filler is deliberately far from `[1, 0, 0, 0]` (the standard query
    /// vector these tests use) along the 4th axis.
    async fn add_filler(view: &VectorView, scope: &Scope, n: usize) {
        for i in 0..n {
            let theta = i as f32 * 0.4;
            let m = mem_with(
                "filler",
                scope.clone(),
                vec![theta.cos() * 0.1, theta.sin() * 0.1, 0.0, 1.0],
            );
            view.apply(&entry(Event::MemoryWritten(m))).await.unwrap();
        }
    }

    #[tokio::test]
    async fn write_then_search_round_trip() {
        let view = VectorView::new(4, "test-mock");
        let scope = Scope::global("test");
        add_filler(&view, &scope, 16).await;

        let a = mem_with("a", scope.clone(), vec![1.0, 0.0, 0.0, 0.0]);
        let a_ref = MemoryRef(a.id);
        let b = mem_with("b", scope.clone(), vec![0.0, 1.0, 0.0, 0.0]);

        view.apply(&entry(Event::MemoryWritten(a))).await.unwrap();
        view.apply(&entry(Event::MemoryWritten(b))).await.unwrap();

        // `a` is the exact target of the query vector; it must come back top-1.
        let hits = view.search(&[1.0, 0.0, 0.0, 0.0], 1, &scope).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].memory, a_ref);
        assert_eq!(hits[0].breakdown[0].0, "vector");
        assert!(hits[0].score > 0.0);
    }

    #[tokio::test]
    async fn invalidate_tombstones_from_search() {
        let view = VectorView::new(4, "test-mock");
        let scope = Scope::global("test");

        add_filler(&view, &scope, 16).await;

        let a = mem_with("a", scope.clone(), vec![1.0, 0.0, 0.0, 0.0]);
        let a_ref = MemoryRef(a.id);
        let b = mem_with("b", scope.clone(), vec![0.95, 0.05, 0.0, 0.0]);
        let b_ref = MemoryRef(b.id);
        view.apply(&entry(Event::MemoryWritten(a))).await.unwrap();
        view.apply(&entry(Event::MemoryWritten(b))).await.unwrap();

        // Sanity: both targets surface before invalidation.
        let before = view.search(&[1.0, 0.0, 0.0, 0.0], 2, &scope).unwrap();
        let before_set: std::collections::HashSet<_> = before.iter().map(|h| h.memory).collect();
        assert!(before_set.contains(&a_ref));
        assert!(before_set.contains(&b_ref));

        view.apply(&entry(Event::MemoryInvalidated {
            id: a_ref,
            reason: "test".into(),
        }))
        .await
        .unwrap();

        // After invalidation, no result should ever be `a_ref`, no matter how
        // many we ask for.
        let after = view.search(&[1.0, 0.0, 0.0, 0.0], 8, &scope).unwrap();
        let after_set: std::collections::HashSet<_> = after.iter().map(|h| h.memory).collect();
        assert!(
            !after_set.contains(&a_ref),
            "invalidated memory must not appear in search results"
        );
        assert!(after_set.contains(&b_ref), "b must still be retrievable");
    }

    #[tokio::test]
    async fn evolved_tombstones_parent_slot() {
        let view = VectorView::new(4, "test-mock");
        let scope = Scope::global("test");
        add_filler(&view, &scope, 16).await;

        let parent = mem_with("v1", scope.clone(), vec![1.0, 0.0, 0.0, 0.0]);
        let parent_ref = MemoryRef(parent.id);
        let mut child = mem_with("v2", scope.clone(), vec![0.99, 0.01, 0.0, 0.0]);
        child.parent = Some(parent_ref);
        let child_ref = MemoryRef(child.id);

        view.apply(&entry(Event::MemoryWritten(parent)))
            .await
            .unwrap();
        view.apply(&entry(Event::MemoryWritten(child)))
            .await
            .unwrap();
        view.apply(&entry(Event::MemoryEvolved {
            from: parent_ref,
            to: child_ref,
            diff: ChangeSet {
                keywords_added: vec![],
                keywords_removed: vec![],
                tags_added: vec![],
                tags_removed: vec![],
                context_rewritten: false,
            },
        }))
        .await
        .unwrap();

        // Over-fetch generously so we'd see the parent if it weren't tombstoned.
        let hits = view.search(&[1.0, 0.0, 0.0, 0.0], 8, &scope).unwrap();
        let memories: Vec<_> = hits.iter().map(|h| h.memory).collect();
        assert!(
            !memories.contains(&parent_ref),
            "parent must be tombstoned after evolution"
        );
        assert!(
            memories.contains(&child_ref),
            "child must remain visible after evolution"
        );
    }

    #[tokio::test]
    async fn scope_filter_refuses_cross_tenant_hits() {
        let view = VectorView::new(4, "test-mock");
        let acme = Scope::global("acme");
        let other = Scope::global("other");

        let a = mem_with("a", acme.clone(), vec![1.0, 0.0, 0.0, 0.0]);
        view.apply(&entry(Event::MemoryWritten(a))).await.unwrap();

        let hits = view.search(&[1.0, 0.0, 0.0, 0.0], 1, &other).unwrap();
        assert!(hits.is_empty(), "cross-tenant hits must be filtered out");
    }

    #[tokio::test]
    async fn skip_insert_when_embedding_missing() {
        let view = VectorView::new(4, "test-mock");
        let scope = Scope::global("test");
        let mut m = mem_with("a", scope.clone(), vec![1.0, 0.0, 0.0, 0.0]);
        m.embedding = None;
        view.apply(&entry(Event::MemoryWritten(m))).await.unwrap();
        let hits = view.search(&[1.0, 0.0, 0.0, 0.0], 1, &scope).unwrap();
        assert!(hits.is_empty());
    }

    #[tokio::test]
    async fn dim_mismatch_on_apply_is_error() {
        let view = VectorView::new(4, "test-mock");
        let scope = Scope::global("test");
        let m = mem_with("a", scope, vec![1.0, 0.0]); // wrong dim
        let res = view.apply(&entry(Event::MemoryWritten(m))).await;
        assert!(res.is_err());
    }

    #[tokio::test]
    async fn dim_mismatch_on_search_is_error() {
        let view = VectorView::new(4, "test-mock");
        let scope = Scope::global("test");
        let res = view.search(&[1.0, 0.0], 1, &scope);
        assert!(res.is_err());
    }

    #[tokio::test]
    async fn async_embedded_round_trip() {
        let view = VectorView::new(4, "test-mock");
        let scope = Scope::global("test");
        add_filler(&view, &scope, 16).await;

        // Write a memory *without* an embedding — the async path.
        let mut a = mem_with("a", scope.clone(), vec![1.0, 0.0, 0.0, 0.0]);
        let a_id = a.id;
        a.embedding = None;
        view.apply(&entry(Event::MemoryWritten(a))).await.unwrap();

        // It must NOT be searchable yet — the hnsw slot doesn't exist.
        let hits = view.search(&[1.0, 0.0, 0.0, 0.0], 5, &scope).unwrap();
        assert!(hits.iter().all(|h| h.memory != MemoryRef(a_id)));

        // Now the embedding worker (simulated) lands a MemoryEmbedded event.
        view.apply(&entry(Event::MemoryEmbedded {
            id: MemoryRef(a_id),
            embedding: vec![1.0, 0.0, 0.0, 0.0],
            model_id: "test-mock".into(),
        }))
        .await
        .unwrap();

        // Now the memory must be searchable.
        let hits = view.search(&[1.0, 0.0, 0.0, 0.0], 1, &scope).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].memory, MemoryRef(a_id));
    }

    #[tokio::test]
    async fn embedded_with_mismatched_model_id_is_refused() {
        let view = VectorView::new(4, "test-mock");
        let scope = Scope::global("test");
        let mut a = mem_with("a", scope.clone(), vec![1.0, 0.0, 0.0, 0.0]);
        let a_id = a.id;
        a.embedding = None;
        view.apply(&entry(Event::MemoryWritten(a))).await.unwrap();

        view.apply(&entry(Event::MemoryEmbedded {
            id: MemoryRef(a_id),
            embedding: vec![1.0, 0.0, 0.0, 0.0],
            model_id: "wrong-model".into(),
        }))
        .await
        .unwrap();

        let hits = view.search(&[1.0, 0.0, 0.0, 0.0], 5, &scope).unwrap();
        assert!(
            hits.iter().all(|h| h.memory != MemoryRef(a_id)),
            "memory embedded with a different model_id must not be searchable"
        );
    }

    #[tokio::test]
    async fn invalidate_drops_pending_embedding() {
        let view = VectorView::new(4, "test-mock");
        let scope = Scope::global("test");
        let mut a = mem_with("a", scope.clone(), vec![1.0, 0.0, 0.0, 0.0]);
        let a_id = a.id;
        a.embedding = None;
        view.apply(&entry(Event::MemoryWritten(a))).await.unwrap();
        view.apply(&entry(Event::MemoryInvalidated {
            id: MemoryRef(a_id),
            reason: "test".into(),
        }))
        .await
        .unwrap();
        // A late MemoryEmbedded should now be ignored.
        view.apply(&entry(Event::MemoryEmbedded {
            id: MemoryRef(a_id),
            embedding: vec![1.0, 0.0, 0.0, 0.0],
            model_id: "test-mock".into(),
        }))
        .await
        .unwrap();
        let hits = view.search(&[1.0, 0.0, 0.0, 0.0], 5, &scope).unwrap();
        assert!(hits.iter().all(|h| h.memory != MemoryRef(a_id)));
    }

    #[tokio::test]
    async fn checkpoint_advances_after_apply() {
        let view = VectorView::new(4, "test-mock");
        assert!(view.checkpoint().await.unwrap().is_none());

        let scope = Scope::global("test");
        let m = mem_with("a", scope, vec![1.0, 0.0, 0.0, 0.0]);
        let e = entry(Event::MemoryWritten(m));
        let id = e.id;
        view.apply(&e).await.unwrap();

        assert_eq!(view.checkpoint().await.unwrap(), Some(id));
    }
}
