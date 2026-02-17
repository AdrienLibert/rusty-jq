# 🦀 rusty-jq

A **blazing-fast** jq-like JSON query engine for Python, written in Rust.

`rusty-jq` compiles jq filter expressions into an optimized Rust pipeline and processes JSON using [simd-json](https://github.com/simd-lite/simd-json) for SIMD-accelerated parsing — delivering **lower latency** than the standard `jq` Python bindings.

---

## ✨ Features

- **jq-compatible syntax** — familiar `.field`, `.[n]`, `.[]`, pipe `|`, and object construction `{}`
- **Zero-copy where possible** — uses `Cow` semantics to avoid unnecessary allocations
- **Compile-once, run-many** — pre-compile queries and reuse them across inputs
- **Native Python types** — results are returned as plain `dict`, `list`, `str`, `int`, `float`, etc.

---

## 🚀 Installation

### Prerequisites

- Python ≥ 3.7
- Rust toolchain (for building from source)
- [Maturin](https://github.com/PyO3/maturin)

### Supported Filters

| Filter | Syntax | Description |
|---|---|---|
| **Identity** | `.` | Returns the input unchanged |
| **Field access** | `.field` | Select a key from an object |
| **Index** | `.[n]` | Access an array element (supports negative indices) |
| **Iterator** | `.[]` | Iterate over all elements of an array |
| **Pipe** | `\|` | Chain filters together |
| **Object construction** | `{key: .field}` | Build a new object from selected fields |

---

## 🏗️ Architecture

| Module | Role |
|---|---|
| `lib.rs` | PyO3 bindings — exposes `compile()` and `JqProgram.input()` to Python |
| `parser.rs` | Query parser built with [nom](https://github.com/rust-bakery/nom) — tokenizes jq expressions into a `Vec<JrFilter>` |
| `engine.rs` | Execution engine — walks the parsed filter chain over `simd_json::BorrowedValue` using `Cow` for zero-copy traversal |

---
