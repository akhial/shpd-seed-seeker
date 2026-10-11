#!/usr/bin/env python3
"""Refresh vault door atlas choices using the official custom tile selectors.

Build the level_map Rust example before running. Its terrain is validated by
the separate official generation fixtures; this compares rendering selection.
"""
import json
import os
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[2]
ORACLE = ROOT / "tooling/oracle-4.0"
env = dict(os.environ, ORACLE_MAIN_CLASS="com.shatteredpixel.shatteredpixeldungeon.VaultDoorOracle")
samples = []
for seed, depth in (("AAA-AAA-AAA", 19), ("AAA-AAA-AAB", 18), ("AAA-AAA-ABG", 17)):
    request = json.dumps(dict(seed=seed, depth=depth, branch=1))
    raw = subprocess.check_output([str(ROOT / "target/release/examples/level_map"), request])
    # Reuse the runner's JDK selection and Windows/MSYS classpath handling.
    # Stdin also avoids translating a temporary file path for a Windows JVM.
    output = subprocess.check_output([str(ORACLE / "run.sh")], input=raw, env=env)
    samples.append(json.loads(output))
fixture = dict(source="Official ShatteredPD-v4.0.2-Java.jar custom vault door selectors", samples=samples)
(ROOT / "crates/seedfinder-core/tests/fixtures/vault-doors.json").write_text(json.dumps(fixture, indent=2) + "\n")
