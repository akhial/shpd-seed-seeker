#!/usr/bin/env python3
"""Summarize probability_sweep JSON; --check fails on material divergence.

Comparisons use a four-standard-error Wilson interval expanded by 35% model
error. Queries with fewer than 30 observed AND predicted hits are marked as
underpowered, not counted as successes. Zero-hit overestimates are checked.
"""
import argparse
import collections
import json
import math
import sys


def assess(hits, samples, estimate):
    if estimate is None or not math.isfinite(estimate) or not 0 <= estimate <= 1:
        return "unavailable"
    if max(hits, samples * estimate) < 30:
        return "underpowered"
    p = hits / samples
    z2 = 16
    center = (p + z2 / (2 * samples)) / (1 + z2 / samples)
    half = 4 * math.sqrt(p * (1 - p) / samples + z2 / (4 * samples**2)) / (1 + z2 / samples)
    return "pass" if (center - half) / 1.35 <= estimate <= min(1, (center + half) * 1.35) else "divergent"


def summarize(report):
    groups = collections.defaultdict(list)
    failures = []
    for row in report["cases"]:
        groups[row["category"]].append(row)
        if assess(row["hits"], report["samples"], row["estimate"]) == "divergent":
            failures.append(row)
    summary = {}
    for name, rows in sorted(groups.items()):
        ratios = sorted(row["estimate"] * report["samples"] / row["hits"] for row in rows if row["hits"] >= 30 and row["estimate"] is not None)
        stats = dict(collections.Counter(assess(row["hits"], report["samples"], row["estimate"]) for row in rows))
        if ratios:
            stats.update({"median_ratio": ratios[len(ratios)//2], "p10_ratio": ratios[int((len(ratios)-1)*.1)], "p90_ratio": ratios[int((len(ratios)-1)*.9)], "mean_absolute_log_error": sum(abs(math.log(max(r,1e-30))) for r in ratios)/len(ratios)})
        summary[name] = stats
    timings = sorted(row["estimate_ns"] for row in report["cases"] if "estimate_ns" in row)
    return {"profile": report["profile"], "samples": report["samples"], "queries": len(report["cases"]), "categories": summary, "timing_ns": {"p50": timings[len(timings)//2], "p95": timings[int((len(timings)-1)*.95)], "max": max(timings)} if timings else None, "divergences": failures}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("reports", nargs="+")
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    summaries = [summarize(json.load(open(path, encoding="utf-8"))) for path in args.reports]
    json.dump(summaries, sys.stdout, indent=2)
    print()
    if args.check and any(report["divergences"] or any(stats.get("unavailable") for stats in report["categories"].values()) for report in summaries):
        sys.exit(1)


if __name__ == "__main__":
    main()
