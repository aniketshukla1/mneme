<div align="center">
  <h1>🧠 mneme</h1>
  <p><strong>A self-improving long-term memory layer for AI agents, built in Rust.</strong></p>

  <p>
    <a href="https://github.com/aniketshukla1/mneme/actions"><img alt="Build Status" src="https://img.shields.io/badge/build-passing-brightgreen"></a>
    <a href="https://crates.io/crates/mneme"><img alt="Version" src="https://img.shields.io/badge/version-v0.1.0-blue"></a>
    <a href="https://github.com/aniketshukla1/mneme/blob/main/LICENSE"><img alt="License" src="https://img.shields.io/badge/license-Apache--2.0-blue.svg"></a>
    <a href="#"><img alt="Tests" src="https://img.shields.io/badge/tests-218%20passing-brightgreen"></a>
  </p>
</div>

---

Most agent memory systems (Mem0, Zep, Letta, Cognee) are **storage-shaped**: they remember facts. mneme's differentiator is **procedural self-improvement** — the agent gets *better at doing things* over time, with a regression guard the literature omits.

Two continuous loops operate over a single append-only event log:

1. **Procedural-memory compiler** (the wedge) — turns batches of agent `Outcome`s into improved, versioned `PolicyArtifact`s (system prompts, heuristics, retrieval rules) via a GEPA-style reflective loop: reflect → propose K candidates → shadow-evaluate → Pareto-select → **gated commit**.
2. **Memory evolution** (supporting) — when a memory is written, a bounded async worker retroactively re-tags and re-links related memories (A-MEM style), keeping the knowledge graph the compiler learns from adaptive.

### Why the wedge matters

> **Hard Rule #1: Nothing procedural commits without passing `EvalReport::is_committable()`** — canaries 100%, safety probe passing, objective Δ ≥ 0. This is the regression guard LangMem omits. Mechanically enforced — setting every configurable gate threshold to its weakest value *still* cannot bypass the baseline. Held by a dedicated integration test on every commit.

---

## ⚡ Quick start

```bash
git clone https://github.com/aniketshukla1/mneme.git
cd mneme

# Run the workspace tests (218 passing)
cargo test --workspace

# Boot the demo: live retrieval + memory evolution + procedural compiler
MNEME_DEMO=1 MNEME_PROCEDURAL=on cargo run -p mneme-server
```

Then open:
- **http://127.0.0.1:7777/** — live chat-style retrieval (hybrid vector + BM25 + extractive synthesis)
- **http://127.0.0.1:7777/dashboard** — real-time benchmarks: latency, BM25 tier distribution, memory evolution chains, **procedural learning curve**

In the demo, watch the **PROCEDURAL** section's learning curve climb from ~33% to 100% while the safety probe line stays glued at 100% — that's the Phase 2 "done when" criterion satisfied live.

### Configuration

| Env var | Default | Meaning |
|---|---|---|
| `MNEME_DEMO` | `0` | `1` → use a temp data dir + synthetic writer (no persistence) |
| `MNEME_EMBEDDER` | `fastembed` | `mock` for a 32-dim deterministic embedder (no model download) |
| `MNEME_EVOLVE` | `on` | `off` to disable the memory-evolution worker |
| `MNEME_PROCEDURAL` | `off` | `on` to enable the procedural compiler (LLM-heavy) |
| `MNEME_EVOLVE_LLM` | `demo` | `ollama` for a real local model via `MNEME_OLLAMA_URL` / `MNEME_OLLAMA_MODEL` |
| `MNEME_DATA` | `./mneme-data` | Path to the fjall keyspace |
| `MNEME_PORT` | `7777` | HTTP listen port |

---

## 🏗️ Workspace architecture

Six crates, each with a focused responsibility. External dependencies sit behind traits (`LlmClient`, `Embedder`, `EventLog`, `MaterializedView`, `Retriever`, `Synthesizer`, `PolicyExecutor`, `Judge`) so providers are swappable.

| Crate | Role | Status |
|---|---|---|
| `mneme-core` | Types + traits + event log shape. No I/O. | ✅ |
| `mneme-store` | `fjall`-backed append-only event log | ✅ Phase 0 |
| `mneme-index` | `hnsw_rs` vector + `tantivy` BM25 + RRF hybrid + extractive synthesis | ✅ Phase 0 |
| `mneme-llm` | `LlmClient` implementations: `FakeLlmClient`, `OllamaLlmClient` (feature-gated) | ✅ |
| `mneme-evolve` | Bounded A-MEM-style memory evolution worker | ✅ Phase 1 |
| `mneme-procedural` | GEPA-style procedural compiler + gate + eval suite + learning curve | ✅ Phase 2 |
| `mneme-server` | Host process: HTTP API, dashboard, demo wiring | ✅ |

---

## 🔒 Hard rules (non-negotiable invariants)

These are enforced in code, not by convention. Each has a dedicated test that fails if the invariant breaks:

1. **Nothing procedural commits without passing `EvalReport::is_committable()`** — canaries + safety probe + non-negative objective delta. The configurable `EvalGates` layer can only add rejection reasons on top of this baseline; it can never relax it. Test: `loosening_configurable_gates_cannot_bypass_strict_baseline`.
2. **Never overwrite history** — memory evolution invalidates the old version and writes a new bi-temporal version with a `parent` pointer. Same for any fact update. The event log is append-only.
3. **Scope is a security boundary** — procedural learning + memory evolution never cross a `Scope` without explicit aggregation. Every cross-entity read goes through `Scope::contains`.
4. **The event log is the single system of record** — every index (vector, BM25, graph, procedural) is a materialized view, fully reconstructible by replaying events. Tested end-to-end.
5. **The write path stays fast** — embedding, evolution, and procedural compilation are async behind bounded queues. The write path target is < 5 ms; LLM calls never block it.
6. **Cascades are bounded** — `EvolveConfig` caps cascade fan-out, per-memory cooldown, lifetime evolution count, and minimum structural delta. A-MEM has no convergence guarantee; these bounds replace it.

---

## 📊 Phase 2 "done when" — verified

> Phase 2 is "done when" the system demonstrates a *positive learning curve on an ALFWorld-style suite with no safety-probe regression*.

Live demo output (`MNEME_PROCEDURAL=on`):

```
v1:  benchmark=33.33% safety=100%
v2:  benchmark=66.67% safety=100%
v3:  benchmark=100.00% safety=100%
v4+: benchmark=100.00% safety=100%  (plateau — both improvement signals integrated)
```

The dashboard renders this as a dual-line chart with a `safety 100%` pill that flips red on any regression.

---

## 🗺️ Roadmap

- **Phase 0** ✅ Foundation — event log, hybrid retrieval, dashboard
- **Phase 1** ✅ Memory evolution — bounded A-MEM-style worker
- **Phase 2** ✅ Procedural compiler — the wedge, with mechanically-enforced commit gate
- **Phase 3** ⏭ Custom filtered HNSW (ACORN-style, soft-delete)
- **Phase 4** ⏭ Bi-temporal property graph store on fjall
- **Phase 5** ⏭ `pyo3` bindings + MCP (Model Context Protocol) server
- **Phase 6** ⏭ Eval harness as a first-class product (the real moat)

---

## 🧪 Test counts

```
mneme-core        :   3 tests
mneme-llm         :  27 tests
mneme-index       :  57 tests
mneme-evolve      :  11 tests
mneme-procedural  :  94 unit + 5 integration tests
mneme-server      :  20 tests
mneme-store       :   1 test
──────────────────────────────
TOTAL             : 218 tests · all passing on both default and --no-default-features
```

---

## 🤝 Contributing

Contributions welcome. A few specific patterns the project enforces:

- **The gate is sacred.** Any change to `mneme-procedural::gate` requires a corresponding test demonstrating that the property still holds. Loosening default thresholds requires a code review comment explaining the trade-off.
- **External dependencies behind traits.** New backends (LLMs, embedders, judges, executors) go behind the existing trait surface; concrete implementations live in their own crate.
- **`cargo fmt` + `cargo clippy -- -D warnings` must pass** on both `--no-default-features` and the default config before any commit.
- **Tests live next to code** in `#[cfg(test)] mod tests`. Storage tests use a temp dir keyed by a fresh ULID and clean up after themselves.
- **Conventional commits** — `feat:`, `fix:`, `refactor:`, `test:`, `docs:`.

---

## 📚 Background

Two design documents back this project:
1. A comparative survey of agent memory systems (Mem0, Zep, Letta, A-MEM, etc.) and where each falls short.
2. The Rust-native self-improving memory architecture + phased build plan.

Section numbers in code comments (e.g. "report §3") refer to document 2.

References embedded in the code:
- A-MEM: Lyu et al., *Agentic Memory for LLM Agents*, arXiv:2502.12110 (memory evolution model)
- GEPA: Du et al., *General Evolutionary Prompt Adaptation*, arXiv:2507.19457 (reflective-loop pattern)
- ACORN: Wu et al., *ACORN: Performant Hybrid Search* (filtered HNSW, Phase 3 target)

---

## ⚠️ Stability

This is `0.1.0` — the first usable release. The system is end-to-end working with 218 passing tests, but the public API surface will change as Phases 3–6 land. Pin a specific version in your `Cargo.toml`; expect breaking changes between `0.x.y` bumps.

---

## 📜 License

Apache License 2.0. See [LICENSE](LICENSE).
