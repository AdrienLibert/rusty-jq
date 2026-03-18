import pytest
import json
import rusty_jq
import jq

@pytest.fixture
def complex_data():
    return {
        "metadata": {
            "source": "payment_gateway",
            "timestamp": 1700000000
        },
        "users": [
            {
                "id": 1,
                "name": "John",
                "profile": {"title": "Data Engineer", "location": "Hong Kong"},
                "transactions": [
                    {"id": 101, "amount": 500, "currency": "HKD"},
                    {"id": 102, "amount": 1200, "currency": "USD"}
                ]
            },
            {
                "id": 2,
                "name": "Bob",
                "profile": {"title": "Manager", "location": "London"},
                "transactions": []
            }
        ]
    }

@pytest.fixture
def json_string(complex_data):
    return json.dumps(complex_data)

@pytest.mark.parametrize("query,expected", [
    # 1. Deep Dive
    (".users | .[0] | .profile | .location", "Hong Kong"),
    
    # 2. Negative Indexing + Pipe
    (".users | .[0] | .transactions | .[-1] | .amount", 1200),
    
    # 3. Empty Array Handling
    (".users | .[1] | .transactions | .[0]", None),
    
    # 4. Safety Check
    (".metadata | .source | .something", None),
    
    # 5. Root Object Access
    (".metadata | .timestamp", 1700000000),

    # 6. Iterator
    (".users | .[] | .id", [1, 2]),
    
    # 7. Simple Select, keep only the user with ID 1
    (".users | .[] | select(.id == 1) | .name", ["John"]),

    # 8. Select with nested path, keep only the user living in London
    (".users | .[] | select(.profile.location == \"London\") | .name", ["Bob"]),

    # 9. Select inside an array with math, find transactions over $1000
    (".users | .[0] | .transactions | .[] | select(.amount > 1000) | .currency", ["USD"]),

    # 10. Select matching nothing, look for an ID that doesn't exist
    (".users | .[] | select(.id == 999) | .name", []),

    # 11. Select with bool path + and
    (".users | .[0] | .transactions | .[] | select(.amount > 100 and .currency == \"USD\") | .id", [102]),

    # 12. Select with or + parenthesized grouping
    (".users | .[] | select(.id == 1 or .profile.location == \"London\") | .name", ["John", "Bob"]),

    # 13. Comma operator — multiple outputs per element
    (".users | .[] | .name, .id", ["John", 1, "Bob", 2]),

    # 14. Parenthesized grouping changes precedence
    (".users | .[0] | .transactions | .[] | select(.amount > 600 and (.currency == \"HKD\" or .currency == \"USD\")) | .id", [102]),
])

def test_jq_queries(complex_data, json_string, query, expected):
    program = rusty_jq.compile(query)
    
    if isinstance(expected, list):
        assert list(program.input(json_string)) == expected
    else:
        assert program.first(json_string) == expected

@pytest.mark.parametrize("query,expected", [
    # 11. smaller object out of each user
    (".users | .[] | {id: .id, loc: .profile | .location}", [
        {"id": 1, "loc": "Hong Kong"},
        {"id": 2, "loc": "London"},
    ]),

    # 12. nested constructor + selecting fields
    (".users | .[0] | {name: .name, profile: {title: .profile | .title}}", {
        "name": "John",
        "profile": {"title": "Data Engineer"},
    }),

    # 13. constructor from root
    (".metadata | {src: .source, ts: .timestamp}", {"src": "payment_gateway", "ts": 1700000000}),
])
def test_object_constructor_json_only(json_string, query, expected):
    program = rusty_jq.compile(query)

    if isinstance(expected, list):
        assert list(program.input(json_string)) == expected
    else:
        assert program.first(json_string) == expected


# ─── v2.2.0 Features: Builtins, Syntax Extensions, Extended Operators ────────

@pytest.mark.parametrize("query,expected", [
    # --- length ---
    (".users | length", 2),
    (".users | .[0] | .name | length", 4),  # "John"
    (".users | .[0] | .transactions | length", 2),
    (".users | .[1] | .transactions | length", 0),

    # --- type ---
    (".users | .[0] | type", "object"),
    (".metadata | .source | type", "string"),
    (".metadata | .timestamp | type", "number"),
    (".users | type", "array"),

    # --- tostring ---
    (".users | .[0] | .id | tostring", "1"),
    (".users | .[] | .id | tostring", ["1", "2"]),

    # --- ascii_upcase / ascii_downcase ---
    (".users | .[] | .name | ascii_upcase", ["JOHN", "BOB"]),
    (".users | .[] | .name | ascii_downcase", ["john", "bob"]),

    # --- has ---
    (".users | .[0] | has(\"name\")", True),
    (".users | .[0] | has(\"nonexistent\")", False),
    (".users | .[0] | .transactions | .[0] | has(\"currency\")", True),

    # --- startswith / endswith / contains ---
    (".users | .[0] | .name | startswith(\"Jo\")", True),
    (".users | .[0] | .name | endswith(\"hn\")", True),
    (".users | .[0] | .name | contains(\"oh\")", True),
    (".users | .[0] | .name | contains(\"xyz\")", False),

    # --- split / join ---
    (".users | .[0] | .profile | .title | split(\" \") | join(\"-\")", "Data-Engineer"),

    # --- ltrimstr / rtrimstr ---
    (".users | .[0] | .profile | .title | ltrimstr(\"Data \")", "Engineer"),
    (".users | .[0] | .profile | .location | rtrimstr(\" Kong\")", "Hong"),

    # --- reverse ---
    (".users | reverse | .[0] | .name", "Bob"),
    (".users | .[0] | .name | reverse", "nhoJ"),

    # --- floor / ceil / round ---
    (".users | .[0] | .transactions | .[0] | .amount | floor", 500),
    (".users | .[0] | .id | ceil", 1),

    # --- explode / implode ---
    (".users | .[1] | .name | explode | implode", "Bob"),

    # --- Slicing ---
    (".users | .[0:1] | .[0] | .name", "John"),
    (".users | .[-1:] | .[0] | .name", "Bob"),
    (".users | .[0] | .name | .[0:2]", "Jo"),
    (".users | .[0] | .name | .[1:3]", "oh"),

    # --- Modulo ---
    (".users | .[0] | .id % 2", 1),
    (".users | .[1] | .id % 2", 0),
    (".users | .[] | .id % 2", [1, 0]),

    # --- String concatenation with + ---
    (".users | .[0] | .name + \" Doe\"", "John Doe"),

    # --- Arithmetic with builtins ---
    (".users | .[0] | .transactions | length + 1", 3),

    # --- index / rindex ---
    (".users | .[0] | .name | index(\"o\")", 1),
    (".users | .[0] | .profile | .location | index(\" \")", 4),

    # --- Pipeline combining multiple builtins ---
    (".users | .[0] | .profile | .title | split(\" \") | .[0] | ascii_upcase", "DATA"),
    (".users | .[0] | .profile | .title | length", 13),
    (".users | .[] | .name | length", [4, 3]),

    # --- tojson ---
    (".users | .[0] | .id | tojson", "1"),

    # --- Select with builtin conditions ---
    (".users | .[] | select(.name | startswith(\"J\")) | .name", ["John"]),
    (".users | .[] | select(.profile.location | endswith(\"Kong\")) | .id", [1]),
    (".users | .[] | select(.profile.title | contains(\"Engineer\")) | .name", ["John"]),

    # --- Combined: select + builtin transform ---
    (".users | .[] | select(.name | endswith(\"ohn\")) | .profile | .title | ascii_upcase", ["DATA ENGINEER"]),
    (".users | .[] | .profile | .title | split(\" \") | .[0]", ["Data", "Manager"]),
])
def test_v2_features(complex_data, json_string, query, expected):
    program = rusty_jq.compile(query)

    if isinstance(expected, list):
        assert list(program.input(json_string)) == expected
    else:
        assert program.first(json_string) == expected


def test_split_returns_array(json_string):
    """split returns a single array result."""
    result = rusty_jq.compile('.users | .[0] | .profile | .title | split(" ")').first(json_string)
    assert result == ["Data", "Engineer"]


def test_explode_returns_array(json_string):
    """explode returns a single array of codepoints."""
    result = rusty_jq.compile('.users | .[1] | .name | explode').first(json_string)
    assert result == [66, 111, 98]  # B=66, o=111, b=98


def test_recursive_descent_numbers(json_string):
    """Recursive descent finds all numbers in nested structure."""
    results = list(rusty_jq.compile('.. | select(type == "number")').input(json_string))
    assert sorted(results) == sorted([1700000000, 1, 101, 500, 102, 1200, 2])


def test_object_values_iteration(json_string):
    """Object iterator .[] works on objects."""
    results = list(rusty_jq.compile('.metadata | .[]').input(json_string))
    assert sorted(results, key=str) == sorted(["payment_gateway", 1700000000], key=str)


def test_keys_sorted(json_string):
    """keys returns sorted key names."""
    result = rusty_jq.compile('.metadata | keys').first(json_string)
    assert result == ["source", "timestamp"]


def test_to_entries_pipeline(json_string):
    """to_entries | .[] | .key extracts keys."""
    results = list(rusty_jq.compile('.metadata | to_entries | .[] | .key').input(json_string))
    assert sorted(results) == ["source", "timestamp"]


def test_from_entries_roundtrip(json_string):
    """to_entries | from_entries preserves data."""
    result = rusty_jq.compile('.metadata | to_entries | from_entries').first(json_string)
    assert result["source"] == "payment_gateway"
    assert result["timestamp"] == 1700000000
