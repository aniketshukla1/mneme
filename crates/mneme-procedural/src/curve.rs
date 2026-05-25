//! [`LearningCurveCollector`] — the Phase 2 "done when" instrument.
//!
//! CLAUDE.md: *Phase 2 done when: positive learning curve on an
//! ALFWorld-style suite with no safety-probe regression.* This module
//! is what makes that statement falsifiable.
//!
//! Records one [`LearningCurvePoint`] per committed artifact version,
//! capturing the absolute [`EvalSuite`][crate::eval::EvalSuite] score
//! and the safety probe pass rate. The dashboard plots these as a line
//! chart: if the benchmark line trends up *and* the safety line stays
//! at 1.0, the wedge is real.
//!
//! ## Where points come from
//!
//! Pushed by the [`crate::worker::ProceduralWorker`] after every
//! successful commit. The worker:
//! 1. lands the commit through `ProceduralCompiler::apply`
//! 2. re-fetches the now-active artifact from the [`crate::ProceduralStore`]
//! 3. runs the [`crate::eval::EvalSuite`] against it
//! 4. records a [`LearningCurvePoint`] here
//!
//! Replay-from-log isn't supported (yet) because the eval scores
//! aren't stored on the wire — they're computed live. A future slice
//! might add a `LearningCurvePoint` event type so points survive
//! restarts; for now the collector is in-memory and lossy on restart,
//! which is fine for the dashboard.
//!
//! ## Concurrency
//!
//! Internal `RwLock` — same single-writer pattern as
//! [`crate::ProceduralStore`]. The worker is the only thing that
//! mutates; many dashboard readers can pull `points()` concurrently.

use mneme_core::types::ArtifactRef;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

/// One point on the learning curve. Snapshot of the suite + gate
/// signal at the moment a commit landed.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LearningCurvePoint {
    pub artifact_id: ArtifactRef,
    pub version: u32,
    /// Wall-clock (unix ms) of the commit.
    pub timestamp_ms: u64,
    /// Absolute suite score `[0.0, 1.0]` — the curve's Y axis.
    pub benchmark_score: f32,
    /// Safety probe pass rate `[0.0, 1.0]` — must stay 1.0 throughout.
    /// Any dip is the alignment-drift signal that should halt
    /// learning per report §10.
    pub safety_probe_pass_rate: f32,
    /// Gate's per-commit objective Δ — useful overlay so the operator
    /// can see whether the absolute score is rising because the *gate*
    /// is rewarding it, or for unrelated reasons.
    pub objective_delta: f32,
    /// How many distinct judges signed off on this commit — diversity
    /// gate compliance indicator.
    pub judges_consulted: u8,
}

/// Append-only collector of learning curve points. Cloneable handle
/// over an internal `Arc<RwLock<…>>`.
#[derive(Clone, Default)]
pub struct LearningCurveCollector {
    inner: Arc<RwLock<Vec<LearningCurvePoint>>>,
}

impl LearningCurveCollector {
    pub fn new() -> Self {
        Self::default()
    }

    /// Append a new point. Ordered by arrival — the dashboard plots
    /// chronologically.
    pub async fn record(&self, point: LearningCurvePoint) {
        self.inner.write().await.push(point);
    }

    /// Snapshot of every recorded point in arrival order.
    pub async fn points(&self) -> Vec<LearningCurvePoint> {
        self.inner.read().await.clone()
    }

    /// All points for a specific artifact, in arrival order. Used by
    /// the dashboard when filtering per-artifact curves.
    pub async fn points_for(&self, aref: ArtifactRef) -> Vec<LearningCurvePoint> {
        self.inner
            .read()
            .await
            .iter()
            .filter(|p| p.artifact_id == aref)
            .cloned()
            .collect()
    }

    pub async fn len(&self) -> usize {
        self.inner.read().await.len()
    }

    pub async fn is_empty(&self) -> bool {
        self.len().await == 0
    }

    /// True iff every recorded point has `safety_probe_pass_rate ==
    /// 1.0`. The single-bit alignment indicator the report (§10) asks
    /// for — monitoring tools that want to halt learning on any
    /// regression read this.
    pub async fn safety_clean(&self) -> bool {
        self.inner
            .read()
            .await
            .iter()
            .all(|p| p.safety_probe_pass_rate >= 1.0 - 1e-6)
    }

    /// True iff `benchmark_score` is monotone non-decreasing across
    /// every recorded point for a given artifact. The strictest
    /// version of "the curve is going up." Dashboards usually want
    /// the *trend* (last N points up) — this method answers the
    /// stricter, easier-to-falsify question.
    pub async fn strictly_non_decreasing(&self, aref: ArtifactRef) -> bool {
        let points = self.points_for(aref).await;
        let mut prev = -1.0f32;
        for p in &points {
            if p.benchmark_score < prev - 1e-6 {
                return false;
            }
            prev = p.benchmark_score;
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mneme_core::types::new_id;

    fn point(version: u32, score: f32, safety: f32, aref: ArtifactRef) -> LearningCurvePoint {
        LearningCurvePoint {
            artifact_id: aref,
            version,
            timestamp_ms: 1_000_000 + version as u64 * 1000,
            benchmark_score: score,
            safety_probe_pass_rate: safety,
            objective_delta: 0.0,
            judges_consulted: 2,
        }
    }

    #[tokio::test]
    async fn record_and_read_back_in_order() {
        let c = LearningCurveCollector::new();
        let a = ArtifactRef(new_id());
        for v in 1..=3 {
            c.record(point(v, v as f32 / 3.0, 1.0, a)).await;
        }
        let pts = c.points().await;
        assert_eq!(pts.len(), 3);
        assert_eq!(pts[0].version, 1);
        assert_eq!(pts[2].version, 3);
    }

    #[tokio::test]
    async fn points_for_filters_by_artifact() {
        let c = LearningCurveCollector::new();
        let a1 = ArtifactRef(new_id());
        let a2 = ArtifactRef(new_id());
        c.record(point(1, 0.5, 1.0, a1)).await;
        c.record(point(1, 0.6, 1.0, a2)).await;
        c.record(point(2, 0.7, 1.0, a1)).await;
        assert_eq!(c.points_for(a1).await.len(), 2);
        assert_eq!(c.points_for(a2).await.len(), 1);
    }

    #[tokio::test]
    async fn safety_clean_detects_any_dip() {
        let c = LearningCurveCollector::new();
        let a = ArtifactRef(new_id());
        c.record(point(1, 0.5, 1.0, a)).await;
        c.record(point(2, 0.6, 1.0, a)).await;
        assert!(c.safety_clean().await);
        c.record(point(3, 0.7, 0.5, a)).await; // ← regression
        assert!(
            !c.safety_clean().await,
            "any dip below 1.0 must trip the flag"
        );
    }

    #[tokio::test]
    async fn strictly_non_decreasing_rejects_dropoff() {
        let c = LearningCurveCollector::new();
        let a = ArtifactRef(new_id());
        c.record(point(1, 0.3, 1.0, a)).await;
        c.record(point(2, 0.6, 1.0, a)).await;
        c.record(point(3, 0.6, 1.0, a)).await; // plateau OK
        assert!(c.strictly_non_decreasing(a).await);
        c.record(point(4, 0.5, 1.0, a)).await; // dropoff!
        assert!(!c.strictly_non_decreasing(a).await);
    }

    #[tokio::test]
    async fn strictly_non_decreasing_is_per_artifact() {
        // A regression on artifact 2 must NOT count against artifact 1.
        let c = LearningCurveCollector::new();
        let a1 = ArtifactRef(new_id());
        let a2 = ArtifactRef(new_id());
        c.record(point(1, 0.5, 1.0, a1)).await;
        c.record(point(2, 0.7, 1.0, a1)).await;
        c.record(point(1, 0.5, 1.0, a2)).await;
        c.record(point(2, 0.2, 1.0, a2)).await;
        assert!(c.strictly_non_decreasing(a1).await);
        assert!(!c.strictly_non_decreasing(a2).await);
    }

    #[tokio::test]
    async fn empty_collector_satisfies_both_predicates_vacuously() {
        let c = LearningCurveCollector::new();
        assert!(c.is_empty().await);
        assert!(c.safety_clean().await);
        assert!(c.strictly_non_decreasing(ArtifactRef(new_id())).await);
    }
}
