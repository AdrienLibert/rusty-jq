import json
import time
import statistics
import jq
import rusty_jq

DATA = {
    "metadata": {"source": "payment_gateway", "timestamp": 1700000000},
    "users": [
        {
            "id": 1,
            "name": "John",
            "profile": {"title": "Data Engineer", "location": "Hong Kong"},
            "transactions": [
                {"id": 101, "amount": 500, "currency": "HKD"},
                {"id": 102, "amount": 1200, "currency": "USD"},
            ],
        },
        {
            "id": 2,
            "name": "Bob",
            "profile": {"title": "Manager", "location": "London"},
            "transactions": [],
        },
    ] * 10000,
}

JSON_TEXT = json.dumps(DATA)

QUERIES = [
    # Simple Access
    ".metadata | .timestamp",
    # Deep Access
    ".users | .[0] | .profile | .location",
    # Array Slicing + Object Access
    ".users | .[0] | .transactions | .[-1] | .amount",
    # Iterator
    ".users | .[] | .id",
    # Constructor
    ".users | .[] | {user_id: .id, city: .profile | .location}",
]

def bench(name, fn, iters=1000):
    for _ in range(100):
        fn()
        
    times = []
    for _ in range(iters):
        t0 = time.perf_counter()
        fn()
        t1 = time.perf_counter()
        times.append((t1 - t0) * 1000.0) 

    return {
        "mean": statistics.mean(times),
        "median": statistics.median(times),
        "stdev": statistics.stdev(times)
    }

def run_comparison():
    print(f"--- BENCHMARK START ---")
    print(f"Data Size: {len(JSON_TEXT) / 1024:.2f} KB")
    print(f"Users: {len(DATA['users'])}")
    print("-" * 60)

    for query in QUERIES:
        print(f"\nQuery: {query}")

        def run_jq():
            return list(jq.compile(query).input(text=JSON_TEXT))

        def run_rusty():
            return rusty_jq.process(query, JSON_TEXT)

        try:
            res_jq = run_jq()
            res_rust = run_rusty()
            
            if not isinstance(res_rust, list) and res_rust is not None:
                res_rust = [res_rust]
            if res_rust is None: 
                res_rust = []
            
            count_jq = len(res_jq)
            count_rust = len(res_rust)
            
            if count_jq != count_rust:
                print(f"⚠️  MISMATCH! jq found {count_jq} items, rusty found {count_rust}")
        except Exception as e:
            print(f"⚠️  CRASH: {e}")
            continue

        stats_jq = bench("jq", run_jq)
        stats_rust = bench("rusty", run_rusty)

        print(f"  jq (official): {stats_jq['median']:.4f} ms")
        print(f"  rusty_jq     : {stats_rust['median']:.4f} ms")
        
        speedup = stats_jq['median'] / stats_rust['median']
        if speedup > 1:
            print(f"  🚀 RESULT: Rusty is {speedup:.2f}x FASTER")
        else:
            print(f"  🐢 RESULT: Rusty is {1/speedup:.2f}x SLOWER")

if __name__ == "__main__":
    run_comparison()