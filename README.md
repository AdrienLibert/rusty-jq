# 🦀 rusty-jq

A **blazing-fast** jq-like JSON query engine for Python, written in Rust.

`rusty-jq` compiles jq filter expressions into an optimized Rust pipeline and processes JSON using [simd-json] for SIMD-accelerated parsing — delivering **up to 14x lower latency** than the standard `jq` Python bindings.

---

## ✨ Features

- **jq-compatible syntax** — familiar `.field`, `.[n]`, `.[]`, pipe `|`, object construction `{}`, comma `,`, and arithmetic `+ - * / %`.
- **49 built-in functions** — `length`, `keys`, `values`, `type`, `sort`, `reverse`, `flatten`, `unique`, `add`, `min`, `max`, `has()`, `contains()`, `startswith()`, `endswith()`, `split()`, `join()`, `ascii_downcase`, `ascii_upcase`, `tostring`, `tonumber`, `to_entries`, `from_entries`, `tojson`, `fromjson`, `explode`, `implode`, `floor`, `ceil`, `round`, `sqrt`, `fabs`, `not`, `empty`, `recurse`, and more.
- **Recursive descent** — use `..` to recursively descend into all nested values.
- **Array/string slicing** — `.[2:5]`, `.[:-1]`, `.[:3]` with negative index support.
- **Extended operators** — `+` on strings (concat), arrays (concat), objects (merge); `-` on arrays (removal); `null + x = x`.
- **Conditional Filtering** — use `select()` with comparison operators (`==`, `!=`, `>`, `<`, `>=`, `<=`), boolean logic (`and`, `or`, `not`), parenthesized grouping, and built-in conditions like `select(.name | startswith("J"))`.
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

| Query | `jq` (official) | `jaq` (binary) | `rusty_jq` | vs `jq` | vs `jaq` |
| --- | --- | --- | --- | --- | --- |
| `.metadata \| .timestamp` | 87.7 ms | 36.9 ms | **13.5 ms** | 🚀 **6.5x** | 🚀 **2.7x** |
| `.users \| .[0] \| .profile \| .location` | 89.9 ms | 37.9 ms | **14.7 ms** | 🚀 **6.1x** | 🚀 **2.6x** |
| `.users \| .[0] \| .transactions \| .[-1] \| .amount` | 79.1 ms | 31.6 ms | **11.4 ms** | 🚀 **6.9x** | 🚀 **2.8x** |
| `.users \| .[] \| .id` | 93.3 ms | 54.7 ms | **10.2 ms** | 🚀 **9.1x** | 🚀 **5.3x** |
| `.users \| .[] \| {user_id: .id, city: .profile \| .location}` | 117.9 ms | 69.2 ms | **19.8 ms** | 🚀 **6.0x** | 🚀 **3.5x** |
| `.users \| .[] \| select(.id == 1) \| .name` | 99.1 ms | 49.0 ms | **13.2 ms** | 🚀 **7.5x** | 🚀 **3.7x** |
| `.users \| .[] \| select(.id > 0 and .profile.location == "Hong Kong") \| .name` | 123.5 ms | 62.5 ms | **17.4 ms** | 🚀 **7.1x** | 🚀 **3.6x** |
| `.users \| .[] \| select(.id == 1 or .profile.location == "London") \| .name` | 114.0 ms | 55.7 ms | **11.0 ms** | 🚀 **10.3x** | 🚀 **5.1x** |
| `.users \| .[] \| select(.id > 0 and (.profile.location == "Hong Kong" or ...)) \| .name` | 93.9 ms | 55.6 ms | **11.7 ms** | 🚀 **8.1x** | 🚀 **4.8x** |
| `.users \| .[] \| .id * 10 + 1` | 73.3 ms | 40.2 ms | **10.5 ms** | 🚀 **7.0x** | 🚀 **3.8x** |
| `.users \| .[] \| .name, .id` | 73.5 ms | 52.7 ms | **17.2 ms** | 🚀 **4.3x** | 🚀 **3.1x** |
| `.users \| length` | 69.6 ms | 34.9 ms | **9.9 ms** | 🚀 **7.0x** | 🚀 **3.5x** |
| `.users \| .[] \| .name \| ascii_upcase` | 142.7 ms | 39.9 ms | **10.2 ms** | 🚀 **14.1x** | 🚀 **3.9x** |
| `.users \| .[] \| .profile \| keys` | 77.3 ms | 46.0 ms | **13.8 ms** | 🚀 **5.6x** | 🚀 **3.3x** |
| `.users \| .[0:100] \| .[] \| .name` | 62.0 ms | 24.9 ms | **8.2 ms** | 🚀 **7.6x** | 🚀 **3.0x** |
| `.users \| .[] \| .id % 2` | 72.8 ms | 40.3 ms | **10.4 ms** | 🚀 **7.0x** | 🚀 **3.9x** |
| `.users \| .[] \| .name + " Doe"` | 71.9 ms | 40.1 ms | **11.5 ms** | 🚀 **6.2x** | 🚀 **3.5x** |
| `.users \| .[] \| has("name")` | 67.6 ms | 35.7 ms | **9.0 ms** | 🚀 **7.5x** | 🚀 **4.0x** |
| `.users \| .[] \| .profile \| .title \| split(" ") \| join("-")` | 138.4 ms | 96.2 ms | **11.9 ms** | 🚀 **11.6x** | 🚀 **8.1x** |
| `.users \| .[] \| select(.name \| startswith("J")) \| .id` | 75.8 ms | 40.0 ms | **10.4 ms** | 🚀 **7.3x** | 🚀 **3.9x** |

---

## 🔍 Supported Filters

| Filter | Syntax | Description |
| --- | --- | --- |
| **Identity** | `.` | Returns the input unchanged |
| **Field access** | `.field` | Select a key from an object |
| **Index** | `.[n]` | Access an array element (supports negative indices) |
| **Slice** | `.[2:5]`, `.[:3]`, `.[-2:]` | Slice arrays or strings with optional start/end and negative indices |
| **Iterator** | `.[]` | Iterate over all elements of an array or values of an object |
| **Recursive descent** | `..` | Recursively descend into all nested values |
| **Pipe** | `\|` | Chain filters together |
| **Select** | `select(.amount > 10)` | Filter items based on boolean conditions (`==`, `!=`, `>`, `<`, `>=`, `<=`, `and`, `or`, `not`, parenthesized grouping, built-in conditions) |
| **Object construction** | `{key: .field}` | Build a new object from selected fields (Cartesian product semantics) |
| **Comma** | `.name, .age` | Run multiple expressions, concatenating all results |
| **Arithmetic** | `.price + .tax`, `.qty * 2` | `+`, `-`, `*`, `/`, `%` with standard precedence; `+` also concats strings/arrays/objects |

### Built-in Functions

<details>
<summary><strong>36 no-arg builtins</strong></summary>

| Function | Description |
| --- | --- |
| `length` | Array/object/string/number/null length |
| `keys` | Sorted key names (or `[0,1,2,...]` for arrays) |
| `keys_unsorted` | Key names in insertion order |
| `values` | Array of values |
| `type` | Type name: `"null"`, `"boolean"`, `"number"`, `"string"`, `"array"`, `"object"` |
| `reverse` | Reverse array or string |
| `sort` | Sort array (jq total order) |
| `flatten` | Recursively flatten nested arrays |
| `add` | Fold array with `+` (sum numbers, concat strings/arrays, merge objects) |
| `min` / `max` | Minimum / maximum element |
| `unique` | Sorted, deduplicated array |
| `first` / `last` | First / last element of array |
| `not` | Negate truthiness |
| `empty` | Produce zero outputs |
| `tostring` / `tonumber` | Type conversion |
| `to_entries` / `from_entries` | Object ↔ `[{key, value}]` conversion |
| `ascii_downcase` / `ascii_upcase` | Case conversion |
| `tojson` / `fromjson` | JSON encode / decode |
| `explode` / `implode` | String ↔ codepoint array |
| `floor` / `ceil` / `round` | Numeric rounding |
| `sqrt` / `fabs` | Square root / absolute value |
| `nan` / `infinite` | NaN / Infinity constants |
| `isinfinite` / `isnan` / `isnormal` | Numeric classification |
| `recurse` | Recursive descent (equivalent to `..`) |

</details>

<details>
<summary><strong>13 one-arg builtins</strong></summary>

| Function | Example | Description |
| --- | --- | --- |
| `has(k)` | `has("name")`, `has(0)` | Test key/index existence |
| `startswith(s)` | `startswith("http")` | String prefix test |
| `endswith(s)` | `endswith(".json")` | String suffix test |
| `contains(s)` | `contains("foo")` | Substring/recursive containment |
| `inside(s)` | `inside("foobar")` | Inverse of contains |
| `split(s)` | `split(",")` | Split string by separator |
| `join(s)` | `join("-")` | Join array with separator |
| `ltrimstr(s)` | `ltrimstr("http://")` | Remove prefix if present |
| `rtrimstr(s)` | `rtrimstr(".json")` | Remove suffix if present |
| `flatten(n)` | `flatten(1)` | Flatten to depth n |
| `index(s)` | `index("bar")` | First occurrence position |
| `rindex(s)` | `rindex("o")` | Last occurrence position |
| `indices(s)` | `indices("a")` | All occurrence positions |

</details>

---

## 🏗️ Architecture

| Module | Role |
| --- | --- |
| `lib.rs` | PyO3 bindings — exposes `compile()`, `.input()`, and `.first()` to Python |
| `parser.rs` | Query parser built with [nom] — tokenizes jq expressions into a `Vec<RustyFilter>` AST |
| `engine.rs` | Execution engine — walks the parsed filter chain over `simd_json::BorrowedValue` using `Cow` for zero-copy traversal and recursive evaluation |

---