# mneme

A self-improving long-term memory layer for AI agents, in Rust.

Most agent memory systems remember *facts*. mneme also improves *procedures* —
the agent gets better at doing things over time. Two loops over one append-only
event log:

- **Procedural-memory compiler** — turns agent outcomes into improved, versioned
  policy artifacts (prompts, heuristics, skills), gated by mandatory
  shadow-evaluation.
- **Memory evolution** — retroactively re-links and re-tags related memories so
  the knowledge graph stays adaptive.

Status: **early scaffold.** Phase 0 (event log) is in place; vector + BM25
views are next. See `CLAUDE.md` for the architecture and build order.

## Quick start

```bash
cargo run -p mneme-server      # boots the event log, prints status
cargo test                     # runs the event-log round-trip test
```

## Layout

| crate | role | phase |
|---|---|---|
| `mneme-core` | types + traits, no I/O | — |
| `mneme-store` | fjall-backed event log | 0 |
| `mneme-index` | hnsw + tantivy retrieval | 0 |
| `mneme-evolve` | A-MEM-style evolution | 1 |
| `mneme-procedural` | the procedural compiler | 2 |
| `mneme-server` | host process / MCP | 5 |

License: Apache-2.0
