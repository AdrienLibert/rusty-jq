import json
import time
import statistics
import subprocess
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
    ] * 5000,
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
    # Select
    ".users | .[] | select(.id == 1) | .name",
]

def run_jaq_cli(query, json_str):
    """
    Runs jaq binary AND parses the result back to Python.
    """
    process = subprocess.run(
        ["jaq", "-c", query],
        input=json_str,
        text=True,
        capture_output=True
    )
    if process.returncode != 0:
        raise Exception(process.stderr)
    
    output_str = process.stdout.strip()
    if not output_str:
        return None
    
    try:
        return json.loads(output_str) 
    except json.JSONDecodeError:
        return [json.loads(line) for line in output_str.splitlines()]

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

        def run_jaq():
            return run_jaq_cli(query, JSON_TEXT)

        def run_rusty():
            return list(rusty_jq.compile(query).input(JSON_TEXT))
        res_jq = run_jq()
        res_jaq = run_jaq()
        res_rust = run_rusty()
            
        stats_jq = bench("jq", run_jq)
        stats_jaq = bench("jaq", run_jaq)
        stats_rust = bench("rusty", run_rusty)

        print(f"  jq (official): {stats_jq['median']:.4f} ms")
        print(f"  jaq (binary) : {stats_jaq['median']:.4f} ms")
        print(f"  rusty_jq     : {stats_rust['median']:.4f} ms")
        
        speedup = stats_jq['median'] / stats_rust['median']
        speedup_jaq = stats_jaq['median'] / stats_rust['median']
        if speedup > 1:
            print(f"  🚀 RESULT: Rusty is {speedup:.2f}x FASTER")
        else:
            print(f"  🐢 RESULT: Rusty is {1/speedup:.2f}x SLOWER")
        if speedup_jaq > 1:
            print(f"  🚀 RESULT: Rusty is {speedup_jaq:.2f}x FASTER")
        else:
            print(f"  🐢 RESULT: Rusty is {1/speedup_jaq:.2f}x SLOWER")

if __name__ == "__main__":
    run_comparison()