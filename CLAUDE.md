# mneme — project guide for Claude Code

**mneme** is a self-improving long-term memory layer for AI agents, written in Rust.
Open-source, agent-framework agnostic. This file is the source of truth for how to
work on it. Read it fully before making changes.

## The wedge (what makes this different)

Most agent memory systems (Mem0, Zep, Letta, Cognee) are *storage-shaped*: they
remember facts. mneme's differentiator is **procedural self-improvement** — the
agent gets *better at doing things* over time. Two loops:

1. **Procedural-memory compiler** (PRIMARY — `mneme-procedural`). Turns batches of
   agent `Outcome`s into improved, versioned `PolicyArtifact`s (system prompts,
   heuristics, skills, retrieval rules) via a GEPA-style reflective loop:
   reflect → propose K candidates → shadow-evaluate → Pareto-select → gated commit.
2. **Memory evolution** (SUPPORTING — `mneme-evolve`). When a memory is written,
   a bounded async worker retroactively re-tags/re-links related memories
   (A-MEM style), so the knowledge graph the compiler learns from stays adaptive.

## Hard rules — do not violate

1. **Nothing procedural commits without passing `EvalReport::is_committable()`.**
   Canaries + safety probe + non-negative objective delta. This is the regression
   guard the literature (LangMem) omits. No exceptions, no "temporary" bypass.
2. **Never overwrite history.** Memory evolution *invalidates and creates a new
   bi-temporal version* (`parent` pointer + new `BiTemporal`). Same for any fact
   update. The event log is append-only and immutable.
3. **Scope is a security boundary.** Procedural learning and evolution must never
   cross a `Scope` without an explicit aggregation/anonymization step. Use
   `Scope::contains` for every cross-entity read.
4. **The event log is the single system of record.** Every index (vector, BM25,
   graph, procedural) is a materialized view that must be rebuildable by replaying
   events. Never let a view hold state that isn't derivable from the log.
5. **The write path stays fast (<5ms target).** Embedding, evolution, and KGG
   extraction are async, behind bounded queues. Never block a write on an LLM call.
6. **Bound the cascades.** Evolution respects `EvolveConfig` caps
   (`max_evolve_per_write`, cooldown, lifetime limit). A-MEM has no convergence
   guarantee; our bounds are what replace it.
7. Keep every external dependency behind a trait (`LlmClient`, `Embedder`,
   `EventLog`, `MaterializedView`, `Retriever`). Providers are swappable.

## Workspace layout

```
crates/
  mneme-core        types + traits, no I/O. the shared vocabulary.
  mneme-store       fjall-backed event log + view plumbing.       [Phase 0]
  mneme-index       hnsw_rs vector view + tantivy BM25 + hybrid.   [Phase 0]
  mneme-evolve      A-MEM-style bounded evolution worker.          [Phase 1]
  mneme-procedural  the procedural-memory compiler (the wedge).    [Phase 2]
  mneme-server      host process; MCP server later.               [Phase 5]
```

Start every task by reading `mneme-core` — `event.rs` (system of record) and
`traits.rs` (the seams) define everything else.

## Build order — do these in sequence, do not skip ahead

- **Phase 0 — Foundation.** Event log is done (`FjallEventLog`). Next: implement
  `VectorView` (hnsw_rs) and `Bm25View` (tantivy) as `MaterializedView`s driven
  off the event tail, then `HybridRetriever` with RRF fusion + explainable
  `Hit.breakdown`. Done when: write/read memories with vector+BM25+scope filter,
  and indexes rebuild from the log.
- **Phase 1 — Memory evolution.** Implement the three A-MEM LLM steps in
  `EvolutionWorker`, all async, with `EvolveConfig` bounds + bi-temporal
  versioning. Done when: A-MEM-style improvement reproduced with bounds enforced.
- **Phase 2 — Procedural compiler (the wedge).** `PolicyArtifact` versioned store
  with atomic active-version hot-swap; GEPA-style reflect/propose/shadow-eval/
  commit; multi-prompt credit assignment. Done when: positive learning curve on
  an ALFWorld-style suite with no safety-probe regression.
- **Phase 3** — custom filtered HNSW (ACORN-style, soft-delete). Only after the
  wedge is demonstrated.
- **Phase 4** — bi-temporal property graph store on fjall.
- **Phase 5** — pyo3 bindings + MCP server.
- **Phase 6** — eval harness as a first-class product (this is the real moat).

When a task can't show its phase's "done when" criterion, stop and instrument —
don't pile on more features.

## Conventions

- `cargo fmt` + `cargo clippy -- -D warnings` must pass before any commit.
- Every public item gets a doc comment; reference the report section it implements.
- Mark unfinished work with `// TODO(phase-N):` so the build order stays visible.
- Tests live next to code in `#[cfg(test)] mod tests`. Storage tests use a temp
  dir keyed by a fresh ULID and clean up after themselves.
- Conventional commits (`feat:`, `fix:`, `refactor:`, `test:`, `docs:`).

## Known hard problems (don't pretend these are solved)

- **Alignment drift** under self-evolution (the "Alignment Tipping Process").
  Safety probes must be externally maintained; a monotone-decreasing probe-score
  trend is a hard stop on commits.
- **In-context reward hacking** in the feedback loop — use judge diversity and
  retrospective audits.
- **Cascade divergence** in evolution — bounds are necessary, not sufficient; a
  periodic consolidation pass re-anchors evolved notes to their `parent`.

## Architecture references

Two design documents back this project (kept outside the repo):
1. Comparative survey of agent memory systems (Mem0, Zep, Letta, A-MEM, etc).
2. The Rust-native self-improving memory architecture + build plan.

Section numbers in code comments (e.g. "report §3") refer to document 2.
