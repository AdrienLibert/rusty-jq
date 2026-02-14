import pytest
import json
import rusty_jq

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
])
def test_jq_queries(complex_data, json_string, query, expected):
    """
    Runs the query against Python Dict and Native
    """
    assert rusty_jq.process(query, json_string) == expected

@pytest.mark.parametrize("query,expected", [
    # 7. smaller object out of each user
    (".users | .[] | {id: .id, loc: .profile | .location}", [
        {"id": 1, "loc": "Hong Kong"},
        {"id": 2, "loc": "London"},
    ]),

    # 8. nested constructor + selecting fields
    (".users | .[0] | {name: .name, profile: {title: .profile | .title}}", {
        "name": "John",
        "profile": {"title": "Data Engineer"},
    }),

    # 9. constructor from root
    (".metadata | {src: .source, ts: .timestamp}", {"src": "payment_gateway", "ts": 1700000000}),
])

def test_object_constructor_json_only(json_string, query, expected):
    assert rusty_jq.process(query, json_string) == expected
