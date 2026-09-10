#!/usr/bin/env python3
"""Run the prespecified native study, or summarize its paired raw measurements.

Build the Rust example first, then run this script with `run` or `summarize`.
Uses only the Python standard library. Run from the repository root.
"""
import json
import random
import subprocess
import sys
from pathlib import Path

OUTPUT = Path("docs/benchmarks/single-trinket")
CASES = [
    ("ring_might", 32768),
    ("grim_weapon", 16384),
    ("thorns_armor", 32768),
    ("ethereal_chains", 32768),
    ("annoying_weapon", 16384),
    ("grim_runic_blade_plus3_depth19", 32768),
    ("plate_armor", 4096),
    ("fireblast", 4096),
    ("grim_and_might", 16384),
    # Follow-up prompted by web batching: a named +1 enchanted item is rare
    # enough that it should benefit from earlier nonempty delivery batches.
    ("grim_runic_blade_plus1_depth19", 32768),
]


def run():
    OUTPUT.mkdir(parents=True, exist_ok=True)
    for name, count in CASES:
        print("START", name, count, flush=True)
        with (OUTPUT / f"{name}.jsonl").open("w") as out:
            subprocess.run(
                ["target/release/examples/single_trinket_benchmark", "bench",
                 str(count), "100000", name], stdout=out, check=True,
            )
        print("DONE", name, flush=True)


def confidence_interval(blocks):
    # Resample consecutive pairs of chunks, keeping AB/BA timing order together
    # and preserving correlation between the baseline and selected-world hits.
    pairs = []
    for i in range(0, len(blocks), 2):
        pair = blocks[i:i + 2]
        pairs.append((
            sum(b["seconds"][0] for b in pair),
            sum(b["seconds"][1] for b in pair),
            sum(b["cells"][1] + b["cells"][3] for b in pair),
            sum(b["cells"][2] + b["cells"][3] for b in pair),
        ))
    rng = random.Random(20260910)
    savings = []
    for _ in range(2500):
        base_time = auto_time = base_hits = auto_hits = 0
        for _ in pairs:
            bt, at, bh, ah = pairs[rng.randrange(len(pairs))]
            base_time += bt
            auto_time += at
            base_hits += bh
            auto_hits += ah
        if base_hits and auto_hits:
            savings.append(100 * (1 - auto_time * base_hits / (base_time * auto_hits)))
    if len(savings) < 2375:
        return None
    savings.sort()
    return [savings[int(len(savings) * p)] for p in (0.025, 0.975)]


def summarize():
    rows = []
    for path in sorted(OUTPUT.glob("*.jsonl")):
        for line in path.read_text().splitlines():
            d = json.loads(line)
            if "blocks" not in d:
                continue
            bh, ah = d["matches"]
            bt, at = d["seconds"]
            bc, ac = d["cpu_ticks"]
            delivery_blocks = []
            for i in range(0, len(d["blocks"]), 8):
                group = d["blocks"][i:i + 8]
                if sum(b["n"] for b in group) != 256:
                    continue
                delivered = [any(b["cells"][mode + 1] + b["cells"][3] for b in group) for mode in (0, 1)]
                cells = [0, 0, 0, 0]
                cells[int(delivered[0]) + 2 * int(delivered[1])] = 1
                delivery_blocks.append({"seconds": [sum(b["seconds"][mode] for b in group) for mode in (0, 1)], "cells": cells})
            successful_batches = [sum(b["cells"][mode + 1] + b["cells"][3] for b in delivery_blocks) for mode in (0, 1)]
            batch_costs = [sum(b["seconds"][mode] for b in delivery_blocks) for mode in (0, 1)]
            batch_latency = [batch_costs[mode] / successful_batches[mode] if successful_batches[mode] else None for mode in (0, 1)]
            rows.append({
                "file": path.name, "query": d["name"], "policy": d["policy"],
                "count": d["count"], "matches": [bh, ah],
                "new": d["cells"][2], "lost": d["cells"][1],
                "seconds": [bt, at], "cpu_seconds": [bc / d["ticks_per_second"], ac / d["ticks_per_second"]],
                "generation_cost_change_percent": 100 * (at / bt - 1),
                "match_increase_percent": 100 * (ah / bh - 1) if bh else None,
                "seconds_per_match": [bt / bh if bh else None, at / ah if ah else None],
                "time_saved_percent": 100 * (1 - at * bh / (bt * ah)) if bh and ah else None,
                "time_saved_95_ci_percent": confidence_interval(d["blocks"]),
                "cpu_time_saved_percent": 100 * (1 - ac * bh / (bc * ah)) if bh and ah else None,
                "setup_seconds": d["setup_seconds"],
                "one_result_seconds_including_setup": [bt / bh if bh else None, d["setup_seconds"] + at / ah if ah else None],
                "ranking": d["ranking"], "selected_cells": d["selected_cells"],
                "native_model_of_256_seed_delivery": {
                    "batch_count": len(delivery_blocks), "successful_batches": successful_batches,
                    "seconds_to_nonempty_batch": batch_latency,
                    "time_saved_percent": 100 * (1 - batch_latency[1] / batch_latency[0]) if all(batch_latency) else None,
                    "time_saved_95_ci_percent": confidence_interval(delivery_blocks),
                    "note": "Aggregated native measurements modeling existing web batch size; not measured browser latency.",
                },
            })
    (OUTPUT / "summary.json").write_text(json.dumps(rows, indent=2) + "\n")
    for r in rows:
        print(r["query"], "n", r["count"], "hits", r["matches"],
              "saved%", r["time_saved_percent"], "95%CI", r["time_saved_95_ci_percent"],
              "CPU saved%", r["cpu_time_saved_percent"], "setup ms", r["setup_seconds"] * 1000)
        print("  256-seed delivery model:", r["native_model_of_256_seed_delivery"])


if __name__ == "__main__":
    if len(sys.argv) == 2 and sys.argv[1] == "run":
        run()
    elif len(sys.argv) == 2 and sys.argv[1] == "summarize":
        summarize()
    else:
        raise SystemExit("Usage: python3 tooling/benchmarks/single_trinket.py run|summarize")
