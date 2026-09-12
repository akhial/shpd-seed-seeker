#!/usr/bin/env python3
"""Compare frozen native binaries sequentially; never build or invoke Java.

Example:
  python3 compare_native.py --baseline-dir /abs/baseline \
    --candidate-dir /abs/candidate --output /abs/new-results \
    --service-module /abs/repo/tooling/benchmarks/effective_matches.py

Each directory contains seed-seeker and match_benchmark (with .exe on Windows).
Adapter comparisons require identical complete recipe/witness records, sorted by
seed. The existing CLI reports only a count, so CLI equality is explicitly
count-only. --seeds overrides the TOTAL seeds per sample for a bounded smoke run;
otherwise fixed case counts are multiplied by --scale and the worker count.
"""

import argparse
from datetime import datetime, timezone
import hashlib
import importlib.util
import json
import math
import os
from pathlib import Path
import platform
import statistics
import subprocess
import sys
import time


DEFAULT_CASES = (
    "cli", "cheap", "early", "wand", "late", "vault_feasible",
    "blade_might", "blade_might_auto", "crossbow",
)
COUNTS = dict(zip(DEFAULT_CASES, (2048, 32768, 8192, 4096, 2048, 2048, 2048, 2048, 2048)))
COUNTS.update(
    cheap_auto=32768, trinket=16384, trinket_deep=4096,
    trinket_or=16384, trinket_mixed=16384, imp_ring=2048,
    imp_greatsword=2048, finite_weapon=2048, finite_armor=2048,
    tier2_melee=2048, tier2_thrown=2048, tier3_heap=2048, tier3_armor_heap=2048,
)
TOTAL_SEEDS = 26 ** 9
CLI_QUERY = {"max_depth": 19, "auto_apply_trinket": False,
             "requirements": [{"item": "runic_blade", "upgrade": 5}]}


def digest(path):
    result = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            result.update(block)
    return result.hexdigest()


def write_json(path, document):
    with path.open("x", encoding="utf-8") as stream:
        json.dump(document, stream, indent=2, allow_nan=False)
        stream.write("\n")


def write_row(stream, row):
    stream.write(json.dumps(row, separators=(",", ":"), allow_nan=False) + "\n")
    stream.flush()


def canonical(document):
    return json.dumps(document, sort_keys=True, separators=(",", ":"), allow_nan=False)


def read_optional(path):
    try:
        return path.read_text(encoding="utf-8")
    except (OSError, UnicodeError):
        return None


def environment_counters():
    """Read-only Linux counters; missing /proc and cgroups are normal on macOS."""
    result = {"utc": datetime.now(timezone.utc).isoformat()}
    for name in ("stat", "loadavg", "pressure/cpu"):
        value = read_optional(Path("/proc") / name)
        if value is not None:
            result["proc/" + name] = value.splitlines()[0] if name == "stat" else value
    for name in ("cpu.stat", "cpu.pressure"):
        value = read_optional(Path("/sys/fs/cgroup") / name)
        if value is not None:
            result["cgroup/" + name] = value
    processes = {}
    for path in Path("/proc").glob("[0-9]*/stat"):
        try:
            value = path.read_text()
            end = value.rindex(")")
            fields = value[end + 1:].split()
            processes[path.parent.name] = {
                "name": value[value.index("(") + 1:end],
                "cpu_jiffies": int(fields[11]) + int(fields[12]),
                "started_jiffies": fields[19],
            }
        except (OSError, ValueError, IndexError):
            continue
    if processes:
        result["processes"] = processes
    return result


def optional_command(command):
    try:
        result = subprocess.run(command, capture_output=True, text=True, timeout=10)
        return {"argv": command, "returncode": result.returncode,
                "stdout": result.stdout, "stderr": result.stderr}
    except (OSError, subprocess.TimeoutExpired) as error:
        return {"argv": command, "unavailable": str(error)}


def find_service_module(explicit):
    if explicit is not None:
        return explicit.resolve(strict=True)
    roots = (Path(__file__).resolve().parent, Path.cwd().resolve())
    for root in roots:
        for parent in (root, *root.parents):
            candidate = parent / "tooling/benchmarks/effective_matches.py"
            if candidate.is_file():
                return candidate
    raise ValueError("cannot locate effective_matches.py; pass --service-module /absolute/path")


def load_service_module(path):
    # Importing Service/seeds_at must not write __pycache__ into a checkout.
    sys.dont_write_bytecode = True
    spec = importlib.util.spec_from_file_location("seed_seeker_effective_matches", path)
    if spec is None or spec.loader is None:
        raise ValueError(f"cannot import {path}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def service_class(module):
    class ManagedService(module.Service):
        """Also clean up when the inherited constructor fails before ready."""
        def __init__(self, command, log):
            self.process = None
            self.log = None
            try:
                super().__init__(command, log)
            except BaseException:
                self.close()
                raise

        def close(self):
            process = self.process
            if process is not None:
                if process.stdin is not None and not process.stdin.closed:
                    try:
                        process.stdin.close()
                    except OSError:
                        pass
                try:
                    process.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait()
                if process.stdout is not None:
                    process.stdout.close()
                self.process = None
            if self.log is not None:
                self.log.close()
                self.log = None
    return ManagedService


def validate_adapter_response(result, seeds):
    if result.get("tested") != len(seeds):
        raise ValueError("adapter tested count differs from submitted seed count")
    matches = result["matches"]
    found = [row["seed"] for row in matches]
    if len(set(found)) != len(found) or not set(found).issubset(seeds):
        raise ValueError("adapter returned duplicate or out-of-interval seeds")
    seconds = result["seconds"]
    if not isinstance(seconds, (int, float)) or not math.isfinite(seconds) or seconds < 0:
        raise ValueError("adapter returned invalid elapsed seconds")
    return sorted(matches, key=lambda row: row["seed"])


def cli_request(binary, count, workers):
    command = [str(binary), "--benchmark", str(count), "--workers", str(workers)]
    process = subprocess.run(command, capture_output=True, text=True, check=True)
    fields = dict(line.split(": ", 1) for line in process.stdout.splitlines() if ": " in line)
    result = {"seconds": float(fields["Elapsed"].split()[0]),
              "tested": int(fields["Seeds tested"]), "matches": int(fields["Matches"]),
              "stdout": process.stdout, "stderr": process.stderr, "argv": command}
    if result["tested"] != count or int(fields["Workers"]) != workers:
        raise ValueError("CLI worker or tested count differs from requested work")
    if not math.isfinite(result["seconds"]) or result["seconds"] < 0:
        raise ValueError("CLI returned invalid elapsed seconds")
    return result


def summarize(rows, cases, workers):
    summary = {"cases": {}, "geomean": {}, "equality_passed": True}
    for worker_count in workers:
        ratios = []
        included = []
        excluded = []
        for case in cases:
            entry = {}
            by_variant = {}
            for variant in ("baseline", "candidate"):
                selected = sorted((row for row in rows if row["workers"] == worker_count
                                   and row["case"] == case and row["variant"] == variant),
                                  key=lambda row: row["rep"])
                by_variant[variant] = selected
                seconds = [row["seconds"] for row in selected]
                total_seconds = sum(seconds)
                entry[variant] = {"seconds": seconds,
                    "wall_seconds": [row["wall_seconds"] for row in selected],
                    "tested": sum(row["tested"] for row in selected),
                    "seeds_per_second": (sum(row["tested"] for row in selected) / total_seconds
                                         if total_seconds > 0 and all(value > 0 for value in seconds)
                                         else None)}
            baseline = entry["baseline"]["seeds_per_second"]
            candidate = entry["candidate"]["seeds_per_second"]
            if baseline is not None and candidate is not None:
                ratio = candidate / baseline
                entry["throughput_ratio"] = ratio
                entry["change_percent"] = 100 * (ratio - 1)
                entry["paired_throughput_ratios"] = [a["seconds"] / b["seconds"] for a, b in
                    zip(by_variant["baseline"], by_variant["candidate"])]
                ratios.append(ratio)
                included.append(case)
            else:
                entry["timing_limitation"] = "A zero/rounded CLI duration prevents a throughput ratio."
                excluded.append(case)
            entry["equality_scope"] = "CLI match count only" if case == "cli" else "complete recipe/witness records"
            summary["cases"][f"{case}/w{worker_count}"] = entry
        ratio = math.exp(statistics.mean(math.log(value) for value in ratios)) if ratios else None
        summary["geomean"][f"w{worker_count}"] = {
            "throughput_ratio": ratio, "change_percent": 100 * (ratio - 1) if ratio is not None else None,
            "included_cases": included, "excluded_cases": excluded,
            "method": "equal-weight geometric mean of per-case total-work/total-internal-time throughput ratios",
        }
    return summary


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--baseline-dir", type=Path, required=True)
    parser.add_argument("--candidate-dir", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True, help="new output directory; existing paths are refused")
    parser.add_argument("--cases", default=",".join(DEFAULT_CASES))
    parser.add_argument("--workers", default="1,8")
    parser.add_argument("--reps", type=int, default=4)
    parser.add_argument("--scale", type=float, default=1)
    parser.add_argument("--start", type=int, default=1_000_000,
                        help="adapter seed-sequence index; measured repetitions must stay within one 26**9 period")
    parser.add_argument("--seeds", type=int, help="override total seeds per sample, including multicore; smoke only")
    parser.add_argument("--workloads", type=Path, default=Path(__file__).with_name("native_workloads.json"))
    parser.add_argument("--service-module", type=Path, help="path to repository effective_matches.py")
    parser.add_argument("--build-metadata", type=Path, help="optional JSON with actual frozen compiler/target/flags provenance")
    args = parser.parse_args()
    cases = args.cases.split(",")
    workers = [int(value) for value in args.workers.split(",")]
    if not all(cases) or len(set(cases)) != len(cases) or not workers or len(set(workers)) != len(workers):
        parser.error("cases and worker counts must be nonempty and unique")
    if any(not all(char.isalnum() or char in "_-" for char in case) for case in cases):
        parser.error("workload names must contain only letters, digits, underscores, or hyphens")
    if min(workers) < 1 or args.reps < 1 or args.start < 0 or not math.isfinite(args.scale) or args.scale <= 0:
        parser.error("workers/reps/scale must be positive and start must be nonnegative")
    if args.seeds is not None and not 1 <= args.seeds <= TOTAL_SEEDS:
        parser.error(f"--seeds must be in 1..={TOTAL_SEEDS}")
    variants = {"baseline": args.baseline_dir, "candidate": args.candidate_dir}
    for name, directory in variants.items():
        if not directory.is_absolute() or not directory.is_dir():
            parser.error(f"{name} directory must be an existing absolute path")
    suite_path = args.workloads.resolve(strict=True)
    suite = json.loads(suite_path.read_text())
    if any(case not in suite or (case not in COUNTS and args.seeds is None) for case in cases):
        parser.error("unknown workload or workload without a fixed count; custom names require --seeds")
    if "cli" in cases and suite["cli"] != CLI_QUERY:
        parser.error("the cli workload must describe the built-in depth-19 Runic Blade +5 benchmark")
    if any(case != "cli" for case in cases) and args.start >= TOTAL_SEEDS:
        parser.error(f"adapter --start must be below {TOTAL_SEEDS}")
    service_path = find_service_module(args.service_module)
    module = load_service_module(service_path)
    Service = service_class(module)
    suffix = ".exe" if os.name == "nt" else ""
    names = (["seed-seeker"] if "cli" in cases else []) + (["match_benchmark"] if any(case != "cli" for case in cases) else [])
    binaries = {name: {kind: directory / (kind + suffix) for kind in names}
                for name, directory in variants.items()}
    for paths in binaries.values():
        for path in paths.values():
            if not path.is_file() or not os.access(path, os.X_OK):
                parser.error(f"missing executable: {path}")
    hashes = {name: {kind: digest(path) for kind, path in paths.items()} for name, paths in binaries.items()}
    sample_counts = {f"{case}/w{worker_count}": (args.seeds if args.seeds is not None else
                     max(4, int(COUNTS[case] * args.scale)) * worker_count)
                     for worker_count in workers for case in cases}
    if max(sample_counts.values()) > TOTAL_SEEDS:
        parser.error("sample exceeds the entire seed space")
    for worker_count in workers:
        for case in cases:
            if case != "cli" and args.start + args.reps * sample_counts[f"{case}/w{worker_count}"] > TOTAL_SEEDS:
                parser.error(f"{case}/w{worker_count} repetitions would wrap the adapter seed-index period; reduce --start, --reps, or sample size")
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    metadata = {
        "started_utc": datetime.now(timezone.utc).isoformat(), "argv": sys.argv,
        "arguments": {key: str(value) if isinstance(value, Path) else value for key, value in vars(args).items()},
        "platform": dict(platform.uname()._asdict()), "python": sys.version, "logical_cpus": os.cpu_count(),
        "host_rustc": optional_command(["rustc", "-vV"]),
        "compiler_provenance_note": "Host rustc is informational; only supplied build metadata identifies frozen artifact compiler/flags.",
        "executable_paths": {name: {kind: str(path) for kind, path in paths.items()} for name, paths in binaries.items()},
        "executable_sha256": hashes,
        "script": {"path": str(Path(__file__).resolve()), "sha256": digest(Path(__file__))},
        "workloads": {"path": str(suite_path), "sha256": digest(suite_path), "queries": {case: suite[case] for case in cases}},
        "service_module": {"path": str(service_path), "sha256": digest(service_path)},
        "sample_counts": sample_counts, "pair_order": "AB, BA, AB, BA; repeat for additional repetitions",
        "timing": "Internal seconds time search work; wall seconds include adapter protocol/output and inherited Service.request response validation. This harness's additional validation, canonicalization, and hashing occur after the wall timer. CLI wall time includes process startup; its internal time includes query planning and worker startup and rounds to milliseconds. Adapter startup/setup are outside samples; each request still starts workers.",
        "seed_order": "CLI scans [0,count) every repetition; adapters use effective_matches.seeds_at(start + rep*count,count). Adapter index intervals must stay within [0,26**9); mapped seed values are intentionally dispersed modulo 26**9.",
        "warmup": "One untimed request per adapter/case/worker with min(256 or 2048 cheap, sample count) seeds; --seeds also bounds warmup. CLI uses one untimed subprocess of min(256,count). No adaptive calibration.",
        "equality": "Exact canonical JSON equality and SHA256 for all adapter recipe/witness records; CLI exposes counts only.",
        "environment_audit": "environment.jsonl contains before/after counters keyed by case, workers, rep, variant; no match witnesses need to be parsed.",
        "limitations": ["No Java/oracle validation is performed.", "Build settings/target CPU must be recorded when freezing binaries; this harness cannot recover them.", "Short --seeds smoke runs establish protocol/equality, not reliable throughput."],
    }
    if platform.system() == "Darwin":
        metadata["macos_hardware"] = optional_command(["sysctl", "machdep.cpu.brand_string", "hw.physicalcpu", "hw.logicalcpu", "hw.memsize"])
    else:
        metadata["proc_cpuinfo"] = read_optional(Path("/proc/cpuinfo"))
    if args.build_metadata is not None:
        metadata["frozen_build_metadata"] = {"path": str(args.build_metadata.resolve()),
            "sha256": digest(args.build_metadata), "document": json.loads(args.build_metadata.read_text())}
    write_json(output / "metadata.json", metadata)
    rows = []
    try:
        with (output / "raw.jsonl").open("x") as raw, (output / "samples.jsonl").open("x") as compact, (output / "comparisons.jsonl").open("x") as comparisons, (output / "environment.jsonl").open("x") as environment:
            for worker_count in workers:
                for case in cases:
                    count = sample_counts[f"{case}/w{worker_count}"]
                    services = {}
                    try:
                        for variant in variants:
                            if case == "cli":
                                cli_request(binaries[variant]["seed-seeker"], min(256, count), worker_count)
                            else:
                                command = [str(binaries[variant]["match_benchmark"]), canonical(suite[case]), str(worker_count)]
                                service = Service(command, output / f"{case}-w{worker_count}-{variant}.log")
                                services[variant] = service
                                warm_seeds = module.seeds_at(9_000_000, min(2048 if case.startswith("cheap") else 256, count))
                                warm = service.request({"seeds": warm_seeds})
                                validate_adapter_response(warm, warm_seeds)
                                write_json(output / f"{case}-w{worker_count}-{variant}-setup.json", {"argv": command, "ready": service.ready, "warmup_seeds": len(warm_seeds), "warmup_seconds": warm["seconds"]})
                        for rep in range(args.reps):
                            order = ["baseline", "candidate"] if rep % 2 == 0 else ["candidate", "baseline"]
                            start = 0 if case == "cli" else args.start + rep * count
                            seeds = None if case == "cli" else module.seeds_at(start, count)
                            compared = {}
                            for variant in order:
                                before = environment_counters()
                                began = time.perf_counter()
                                result = (cli_request(binaries[variant]["seed-seeker"], count, worker_count) if case == "cli" else services[variant].request({"seeds": seeds}))
                                wall = time.perf_counter() - began
                                after = environment_counters()
                                write_row(environment, {"case": case, "workers": worker_count,
                                    "rep": rep, "variant": variant, "start_index": start,
                                    "environment_before": before, "environment_after": after})
                                records = ({"tested": result["tested"], "match_count": result["matches"]} if case == "cli" else validate_adapter_response(result, seeds))
                                serialized = canonical(records)
                                signature = hashlib.sha256(serialized.encode("utf-8")).hexdigest()
                                compared[variant] = (signature, serialized)
                                row = dict(result, variant=variant, case=case, workers=worker_count, rep=rep,
                                    start_index=start, wall_seconds=wall, signature_sha256=signature,
                                    equality_scope="cli_count_only" if case == "cli" else "complete_recipe_witness_records",
                                    environment_before=before, environment_after=after)
                                write_row(raw, row)
                                small = {key: value for key, value in row.items() if key not in ("matches", "stdout", "stderr", "environment_before", "environment_after")}
                                small["match_count"] = result["matches"] if case == "cli" else len(result["matches"])
                                write_row(compact, small)
                                rows.append(small)
                                print(f"{case} w{worker_count} r{rep} {variant}: {count}/{row['seconds']:.6f}s matches={small['match_count']}", flush=True)
                            equal = compared["baseline"] == compared["candidate"]
                            write_row(comparisons, {"case": case, "workers": worker_count, "rep": rep,
                                "baseline_sha256": compared["baseline"][0], "candidate_sha256": compared["candidate"][0], "equal": equal})
                            if not equal:
                                raise ValueError(f"match equality failed: {case}, workers={worker_count}, rep={rep}; raw records retained")
                    finally:
                        for service in services.values():
                            service.close()
        final_hashes = {name: {kind: digest(path) for kind, path in paths.items()} for name, paths in binaries.items()}
        if hashes != final_hashes:
            raise ValueError("a frozen executable changed during measurement")
        summary = summarize(rows, cases, workers)
        write_json(output / "summary.json", summary)
        write_json(output / "completion.json", {"status": "complete", "executable_hashes_unchanged": True,
            "finished_utc": datetime.now(timezone.utc).isoformat()})
        print(json.dumps(summary, indent=2), flush=True)
    except BaseException as error:
        write_json(output / "failure.json", {"error": str(error), "kind": type(error).__name__,
            "finished_utc": datetime.now(timezone.utc).isoformat()})
        raise


if __name__ == "__main__":
    main()
