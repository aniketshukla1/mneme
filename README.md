<div align="center">
  <h1>🧠 mneme</h1>
  <p><strong>A self-improving long-term memory layer for AI agents, built in Rust.</strong></p>

  <p>
    <a href="https://github.com/aniketshukla1/mneme/actions"><img alt="Build Status" src="https://img.shields.io/badge/build-passing-brightgreen"></a>
    <a href="https://crates.io/crates/mneme"><img alt="Crates.io" src="https://img.shields.io/badge/crates.io-v0.0.1-blue"></a>
    <a href="https://github.com/aniketshukla1/mneme/blob/main/LICENSE"><img alt="License" src="https://img.shields.io/badge/license-Apache--2.0-blue.svg"></a>
  </p>
</div>

---

Most agent memory systems (like Mem0, Zep, Letta, or Cognee) are *storage-shaped*: they remember facts. **mneme's differentiator is procedural self-improvement** — the agent gets *better at doing things* over time. 

Mneme achieves this through two continuous loops operating over a single, append-only event log:

1. **Procedural-Memory Compiler** — Turns batches of agent outcomes into improved, versioned policy artifacts (system prompts, heuristics, skills, retrieval rules). This uses a GEPA-style reflective loop (reflect → propose candidates → shadow-evaluate → Pareto-select → gated commit).
2. **Memory Evolution** — When a new memory is written, a bounded async worker retroactively re-tags and re-links related past memories, ensuring the knowledge graph stays adaptive and contextually rich.

> **Status:** Early Scaffold (Phase 0). The core event log is in place. Vector and BM25 view materialization are up next.

---

## ⚡ Quick Start

Ensure you have [Rust and Cargo](https://rustup.rs/) installed.

```bash
# Clone the repository
git clone https://github.com/aniketshukla1/mneme.git
cd mneme

# Boot the event log server (prints status)
cargo run -p mneme-server

# Run the event-log round-trip tests
cargo test
```

---

## 🏗️ Workspace Architecture

Mneme is divided into several crates, keeping the core logic isolated from I/O and specific implementations.

| Crate | Role | Phase |
|---|---|---|
| `mneme-core` | Core types and traits (No I/O) | — |
| `mneme-store` | **fjall**-backed append-only event log | 0 |
| `mneme-index` | **hnsw** + **tantivy** for vector & text retrieval | 0 |
| `mneme-evolve` | A-MEM-style retroactive memory evolution | 1 |
| `mneme-procedural` | The procedural policy compiler | 2 |
| `mneme-server` | Host process / MCP (Model Context Protocol) | 5 |

### Core Design Tenets
* **Single System of Record:** The append-only event log is the absolute source of truth. Every index (vector, BM25, graph) is a materialized view that can be fully rebuilt by replaying events.
* **Fast Write Path:** Writes are fast (<5ms target). Embedding, evolution, and extraction are handled asynchronously behind bounded queues.
* **Immutable History:** We never overwrite history. Fact updates and memory evolutions invalidate past states by creating new bi-temporal versions.

---

## 🤝 Contributing

We welcome contributions from the community! Whether you're fixing bugs, improving documentation, or proposing new features, your help is appreciated. 

### How to Contribute

1. **Fork the Repository**: Start by forking the project to your own GitHub account.
2. **Create a Branch**: Create a feature branch from `main` (`git checkout -b feature/your-feature-name`).
3. **Make your Changes**: Write your code! Make sure to follow the coding guidelines below.
4. **Test Thoroughly**: Run `cargo test` to ensure your changes don't break existing functionality. Add new tests for any new features.
5. **Commit & Push**: Commit your changes with descriptive commit messages, then push your branch to your fork.
6. **Open a Pull Request**: Submit a PR against the `main` branch of the `mneme` repository. Include a clear description of the problem you're solving or the feature you're adding.

### Coding Guidelines

* **Strict Safety & Commits:** Nothing procedural should commit without passing evaluation (`EvalReport::is_committable()`). This is a hard guard against regression.
* **Dependencies:** Keep external dependencies behind traits (`LlmClient`, `Embedder`, `EventLog`, `Retriever`) so providers remain swappable.
* **Formatting:** All code must be formatted with `cargo fmt`.
* **Linting:** Ensure your code passes all `cargo clippy` checks without warnings.
* **Documentation:** Document new traits, structs, and complex functions. 

### Reporting Issues

If you find a bug or have a feature request, please open an issue! Provide as much context as possible:
* Steps to reproduce the bug.
* Expected vs. actual behavior.
* Environment details (OS, Rust version, etc.).

---

## 📜 License

This project is licensed under the **Apache License 2.0**. See the [LICENSE](LICENSE) file for details.
