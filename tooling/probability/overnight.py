#!/usr/bin/env python3
"""Run independent calibration jobs into an artifact directory, never source files.

Build the seven release examples and the release probability_fuzz test first.
Launch detached; manifest.json records commands, seeds, hashes, and exit codes.
Failed validation is recorded without discarding successful calibration outputs.
"""
import argparse
import datetime
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys

PROFILES = ["none", "mimic_tooth", "parchment_scrap", "rat_skull", "exotic_crystals", "mossy_clump", "trap_mechanism", "cracked_spyglass"]
STRIDE = 3_355_211_884_971
TOTAL_SEEDS = 5_429_503_678_976
FLOOR_SAMPLES = 65_535


def now():
    return datetime.datetime.now(datetime.timezone.utc).isoformat()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path)
    parser.add_argument("--item-worlds", type=int, default=1_000_000)
    parser.add_argument("--floor-shards", type=int, default=16)
    parser.add_argument("--validation-worlds", type=int, default=262_144)
    parser.add_argument("--fuzz-queries", type=int, default=8_000)
    parser.add_argument("--plan", action="store_true")
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[2]
    output = args.output.resolve()
    if not all(value > 0 for value in [args.item_worlds, args.floor_shards, args.validation_worlds, args.fuzz_queries]):
        parser.error("sample and shard counts must be positive")
    names = ["calibrate_probability", "calibrate_floors", "calibrate_first_floor", "calibrate_weapon_repeats", "calibrate_source_counts", "calibrate_wand_repeats", "probability_sweep"]
    binaries = {name: root / "target/release/examples" / name for name in names}
    fuzz = [path for path in (root / "target/release/deps").glob("probability_fuzz-*") if path.is_file() and os.access(path, os.X_OK)]
    if not fuzz or not all(path.is_file() for path in binaries.values()):
        parser.error("build the release examples and probability_fuzz test first")
    binaries["probability_fuzz"] = max(fuzz, key=lambda path: path.stat().st_mtime_ns)
    tools = output / "tools"
    jobs = []
    for profile in PROFILES:
        command = [str(tools / "calibrate_probability"), str(args.item_worlds)]
        if profile != "none":
            command.append(profile)
        jobs.append((f"items-{profile}", command, f"items-{profile}.rs", None, {}))
    jobs.append(("first-floor", [str(tools / "calibrate_first_floor"), "2000000"], "first-floor.rs", None, {}))
    jobs.append(("weapon-repeats", [str(tools / "calibrate_weapon_repeats"), "500000"], "weapon-repeats.rs", None, {}))
    jobs.append(("feeling-rooms", [str(tools / "calibrate_floors"), "524288", str(output / "feeling-rooms.bin.partial"), "1618033988", "exact"], "feeling-rooms.stdout.log", "feeling-rooms.bin", {}))
    jobs.append(("source-counts", [str(tools / "calibrate_source_counts"), "65536", str(output / "source-counts.bin.partial")], "source-counts.stdout.log", "source-counts.bin", {}))
    for profile in PROFILES:
        name = f"wand-repeats-{profile}"
        command = [str(tools / "calibrate_wand_repeats"), "262144", str(output / f"{name}.json.partial")]
        if profile != "none":
            command.append(profile)
        jobs.append((name, command, f"{name}.stdout.log", f"{name}.json", {}))
    jobs.append(("wand-table", [sys.executable, str(tools / "pack_wand_repeats.py"), str(output / "wand-repeats.bin.partial"), *[str(output / f"wand-repeats-{p}.json") for p in PROFILES]], "wand-table.stdout.log", "wand-repeats.bin", {}))
    for shard in range(args.floor_shards):
        # Fresh, nonoverlapping blocks after the checked-in training corpus.
        offset = (17_389 + (shard + 1) * FLOOR_SAMPLES * STRIDE) % TOTAL_SEEDS
        name = f"floors-{shard:02}"
        jobs.append((name, [str(tools / "calibrate_floors"), str(FLOOR_SAMPLES), str(output / f"{name}.bin.partial"), str(offset)], f"{name}.stdout.log", f"{name}.bin", {}))
    for profile in PROFILES:
        # An independent validation stream, distinct from today's audit.
        name = f"validation-{profile}"
        jobs.append((name, [str(tools / "probability_sweep"), str(args.validation_worlds), profile, "2718281828"], f"{name}.json", None, {}))
    jobs.append(("random-queries", [str(tools / "probability_fuzz"), "--exact", "fuzzed_queries_track_sampled_seeds", "--ignored", "--nocapture"], "random-queries.log", None, {"FUZZ_WORLDS": str(args.validation_worlds), "FUZZ_QUERIES": str(args.fuzz_queries), "FUZZ_SEED": "20260922", "FUZZ_REPORT": str(output / "random-queries.json")}))
    jobs.append(("validation-summary", [sys.executable, str(tools / "report.py"), "--check", *[str(output / f"validation-{p}.json") for p in PROFILES]], "validation-summary.json", None, {}))
    if args.plan:
        print(json.dumps(jobs, indent=2))
        return
    if (output / "manifest.json").exists():
        parser.error("refusing to overwrite an existing run")
    tools.mkdir(parents=True, exist_ok=True)
    os.nice(10)
    manifest = {"started": now(), "pid": os.getpid(), "repository": str(root), "git_head": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=root, text=True).strip(), "configuration": {k: str(v) if isinstance(v, Path) else v for k, v in vars(args).items()}, "binaries": {}, "jobs": []}
    for name, source in binaries.items():
        target = tools / name
        shutil.copy2(source, target)
        manifest["binaries"][name] = hashlib.sha256(target.read_bytes()).hexdigest()
    shutil.copy2(Path(__file__).with_name("report.py"), tools / "report.py")
    shutil.copy2(__file__, tools / "overnight.py")
    shutil.copy2(Path(__file__).with_name("pack_wand_repeats.py"), tools / "pack_wand_repeats.py")
    (output / "source-diff.patch").write_bytes(subprocess.check_output(["git", "diff", "--binary"], cwd=root))
    sources = output / "sources"
    sources.mkdir()
    for source in (root / "crates/seedfinder-core/examples").glob("calibrate_*.rs"):
        shutil.copy2(source, sources / source.name)
    shutil.copy2(root / "crates/seedfinder-core/examples/probability_sweep.rs", sources / "probability_sweep.rs")
    for relative in ["probability/floors.rs", "probability_tables/floors.rs", "probability_tables/first_floor.rs", "probability_tables/weapon_repeats.rs", "probability_tables/source_counts.rs", "probability_tables/wand_repeats.rs"]:
        target = sources / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(root / "crates/seedfinder-core/src" / relative, target)
    shutil.copy2(root / "crates/seedfinder-core/tests/probability_fuzz.rs", sources / "probability_fuzz.rs")

    def save():
        temporary = output / "manifest.json.partial"
        temporary.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
        temporary.replace(output / "manifest.json")

    save()
    for name, command, stdout_name, binary_name, overrides in jobs:
        job = {"name": name, "command": command, "environment": overrides, "started": now(), "status": "running"}
        manifest["jobs"].append(job)
        save()
        print(f"{now()} starting {name}", flush=True)
        stdout = output / f"{stdout_name}.partial"
        with stdout.open("wb") as stream, (output / f"{name}.stderr.log").open("wb") as errors:
            result = subprocess.run(command, cwd=root, env={**os.environ, **overrides}, stdout=stream, stderr=errors, check=False)
        job.update({"finished": now(), "exit_code": result.returncode, "status": "complete" if result.returncode == 0 else "failed"})
        # Validation failures still produce useful observations and reports.
        if result.returncode == 0 or name in {"random-queries", "validation-summary"}:
            stdout.replace(output / stdout_name)
        if binary_name and result.returncode == 0:
            (output / f"{binary_name}.partial").replace(output / binary_name)
        save()
        print(f"{now()} {name}: exit {result.returncode}", flush=True)
    manifest["finished"] = now()
    manifest["status"] = "complete" if all(job["exit_code"] == 0 for job in manifest["jobs"]) else "complete_with_failures"
    save()


if __name__ == "__main__":
    main()
