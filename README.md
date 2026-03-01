# 🦀 rusty-jq

A **blazing-fast** jq-like JSON query engine for Python, written in Rust.

`rusty-jq` compiles jq filter expressions into an optimized Rust pipeline and processes JSON using [simd-json] for SIMD-accelerated parsing — delivering **up to 9x lower latency** than the standard `jq` Python bindings.

---

## ✨ Features

- **jq-compatible syntax** — familiar `.field`, `.[n]`, `.[]`, pipe `|`, and object construction `{}`.
- **Conditional Filtering** — use `select()` with recursive mini-queries and comparison operators (`==`, `>`, `<=`, etc.) to filter streams.
- **Zero-copy where possible** — uses `Cow` semantics to avoid unnecessary allocations, keeping the memory footprint tiny.
- **Compile-once, run-many** — pre-compile queries into an AST and reuse them across inputs.
- **Stream or Fast-Path** — return an iterator of all matches using `.input()`, or avoid iterator overhead entirely and grab the single first match with `.first()`.
- **Native Python types** — results are safely and quickly converted back to plain `dict`, `list`, `str`, `int`, `float`, etc.

---

## 🚀 Installation

Requires Python ≥ 3.7.

```bash
pip install rusty-jq

```

---

## 💻 Usage

```python
import rusty_jq
import json

json_data = '{"users": [{"id": 1, "name": "John"}, {"id": 2, "name": "Bob"}]}'

# 1. Compile the query once
program = rusty_jq.compile(".users | .[] | select(.id == 1) | .name")

# 2a. Stream all matches (iterator way)
results = list(program.input(json_data))
print(results) # ["John"]

# 2b. Fast-path, grab only the first match
first_match = program.first(json_data)
print(first_match) # "John"

```

---

## 📊 Benchmarks

`rusty-jq` was benchmarked against the official `jq` Python bindings and the highly optimized `jaq` CLI tool.

**Test Payload:** 1.49 MB JSON file containing 10,000 nested user objects.

| Query | `jq` (official) | `jaq` (binary) | `rusty_jq` | Speedup vs `jq` |
| --- | --- | --- | --- | --- |
| `.metadata \| .timestamp` | 65.4 ms | 24.5 ms | **7.8 ms** | 🚀 **8.4x** |
| `.users \| .[0] \| .profile \| .location` | 63.8 ms | 25.4 ms | **7.1 ms** | 🚀 **8.9x** |
| `.users \| .[0] \| .transactions \| .[-1] \| .amount` | 63.5 ms | 24.4 ms | **7.0 ms** | 🚀 **9.1x** |
| `.users \| .[] \| .id` | 69.9 ms | 39.1 ms | **8.3 ms** | 🚀 **8.4x** |
| `.users \| .[] \| {user_id: .id}` | 96.0 ms | 57.5 ms | **16.9 ms** | 🚀 **5.7x** |
| `.users \| .[] \| select(.id == 1)` | 84.6 ms | 50.0 ms | **10.9 ms** | 🚀 **7.7x** |

---

## 🔍 Supported Filters

| Filter | Syntax | Description |
| --- | --- | --- |
| **Identity** | `.` | Returns the input unchanged |
| **Field access** | `.field` | Select a key from an object |
| **Index** | `.[n]` | Access an array element (supports negative indices) |
| **Iterator** | `.[]` | Iterate over all elements of an array |
| **Pipe** | `\|` | Chain filters together |
| **Select** | `select(.amount > 10)` | Filter items based on boolean conditions (`==`, `!=`, `>`, `<`, `>=`, `<=`) |
| **Object construction** | `{key: .field}` | Build a new object from selected fields |

---

## 🏗️ Architecture

| Module | Role |
| --- | --- |
| `lib.rs` | PyO3 bindings — exposes `compile()`, `.input()`, and `.first()` to Python |
| `parser.rs` | Query parser built with [nom] — tokenizes jq expressions into a `Vec<RustyFilter>` AST |
| `engine.rs` | Execution engine — walks the parsed filter chain over `simd_json::BorrowedValue` using `Cow` for zero-copy traversal and recursive evaluation |

---