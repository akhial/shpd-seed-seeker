#!/usr/bin/env python3
"""Run/summarize the production API benchmark using only the Python stdlib.

First build auto_trinket_production_benchmark in release mode. Run from the
repository root. Historical single-trinket study artifacts are not changed.
"""
import hashlib
import json
import os
import platform
import random
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path

OUTPUT = Path("docs/benchmarks/auto-trinket-production")
BINARY = Path("target/release/examples/auto_trinket_production_benchmark")
CASES = [
    "grim_runic_blade_plus1_depth19",
    "ring_might_plus2_depth9",
    "annoying_weapon_plus1_depth9",
]
COUNT = 32768
START = 300000


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def run():
    OUTPUT.mkdir(parents=True, exist_ok=True)
    sources = [
        "crates/seedfinder-core/examples/auto_trinket_production_benchmark.rs",
        "crates/seedfinder-core/src/auto_trinkets.rs",
        "crates/seedfinder-core/src/feasibility.rs",
        "crates/seedfinder-core/src/main_world.rs",
        "crates/seedfinder-core/src/probability.rs",
    ]
    environment = {
        "started_utc": datetime.now(timezone.utc).isoformat(),
        "platform": platform.platform(), "logical_cpus": os.cpu_count(),
        "rustc": subprocess.check_output(["rustc", "--version"], text=True).strip(),
        "git_base": subprocess.check_output(["git", "rev-parse", "HEAD"], text=True).strip(),
        "note": "Working-tree implementation; source hashes identify measured code. Shared host; single benchmark process.",
        "binary_sha256": digest(BINARY),
        "sources_sha256": {path: digest(path) for path in sources},
        "prespecified_count_per_mode_per_query": COUNT,
        "prespecified_start_index": START,
        "seed_formula": "(index * PRODUCTION_SEARCH_START_STRIDE + 812345678901) % TOTAL_SEEDS",
        "mode_order": "AB/BA alternating blocks of 32 identical seeds",
    }
    (OUTPUT / "environment.json").write_text(json.dumps(environment, indent=2) + "\n")
    for name in CASES:
        print("START", datetime.now(timezone.utc).isoformat(), name, COUNT, flush=True)
        with (OUTPUT / f"{name}.jsonl").open("w") as out:
            subprocess.run([str(BINARY), str(COUNT), str(START), name], stdout=out, check=True)
        print("DONE", datetime.now(timezone.utc).isoformat(), name, flush=True)
    environment["finished_utc"] = datetime.now(timezone.utc).isoformat()
    (OUTPUT / "environment.json").write_text(json.dumps(environment, indent=2) + "\n")


def confidence_interval(blocks):
    # Keep adjacent AB/BA chunks together, and retain paired hit correlation.
    pairs = []
    for i in range(0, len(blocks), 2):
        pair = blocks[i:i + 2]
        pairs.append((
            sum(b["seconds"][0] for b in pair),
            sum(b["seconds"][1] for b in pair),
            sum(b["cells"][1] + b["cells"][3] for b in pair),
            sum(b["cells"][2] + b["cells"][3] for b in pair),
        ))
    rng = random.Random(20260911)
    samples = []
    for _ in range(5000):
        bt = at = bh = ah = 0
        for _ in pairs:
            t0, t1, h0, h1 = pairs[rng.randrange(len(pairs))]
            bt += t0
            at += t1
            bh += h0
            ah += h1
        if bh and ah:
            samples.append(100 * (1 - at * bh / (bt * ah)))
    if len(samples) < 4750:
        return None
    samples.sort()
    return [samples[int(len(samples) * p)] for p in (0.025, 0.975)]


def summarize():
    rows = []
    for path in sorted(OUTPUT.glob("*.jsonl")):
        for line in path.read_text().splitlines():
            d = json.loads(line)
            bt, at = d["seconds"]
            bh, ah = d["matches"]
            bc, ac = d["cpu_ticks"]
            rows.append({
                "file": path.name, "query": d["name"], "count_per_mode": d["count"],
                "matches": [bh, ah], "new_matches": d["cells"][2], "lost_matches": d["cells"][1],
                "seconds": [bt, at], "seeds_per_second": [d["count"] / bt, d["count"] / at],
                "cpu_seconds": [bc / d["ticks_per_second"], ac / d["ticks_per_second"]],
                "seconds_per_match": [bt / bh if bh else None, at / ah if ah else None],
                "matches_per_second": [bh / bt, ah / at],
                "per_seed_cost_change_percent": 100 * (at / bt - 1),
                "time_per_match_saved_percent": 100 * (1 - at * bh / (bt * ah)) if bh and ah else None,
                "time_per_match_saved_95_ci_percent": confidence_interval(d["blocks"]),
                "cpu_time_per_match_saved_percent": 100 * (1 - ac * bh / (bc * ah)) if bh and ah else None,
                "plan_setup_milliseconds": [s * 1000 for s in d["setup_seconds"]],
                "preferred_trinkets": d["ranking"], "validation": d["validation"],
            })
    (OUTPUT / "summary.json").write_text(json.dumps(rows, indent=2) + "\n")
    for row in rows:
        print(json.dumps(row))


if __name__ == "__main__":
    if len(sys.argv) == 2 and sys.argv[1] == "run":
        run()
    elif len(sys.argv) == 2 and sys.argv[1] == "summarize":
        summarize()
    else:
        raise SystemExit("Usage: python3 tooling/benchmarks/auto_trinket_production.py run|summarize")
