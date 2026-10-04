#!/usr/bin/env python3
"""Refresh vault door atlas choices using the official custom tile selectors.

Build the level_map Rust example before running. Its terrain is validated by
the separate official generation fixtures; this compares rendering selection.
"""
import json
import os
from pathlib import Path
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[2]
ORACLE = ROOT / "tooling/oracle-4.0"
subprocess.run([str(ORACLE / "build.sh")], check=True, stdout=subprocess.DEVNULL)
java_home = os.environ.get("JAVA_21_HOME") or os.environ.get("JAVA_HOME")
java = str(Path(java_home) / "bin/java") if java_home else "java"
samples = []
for seed, depth in (("AAA-AAA-AAA", 19), ("AAA-AAA-AAB", 18), ("AAA-AAA-ABG", 17)):
    request = json.dumps(dict(seed=seed, depth=depth, branch=1))
    raw = subprocess.check_output([str(ROOT / "target/release/examples/level_map"), request])
    with tempfile.NamedTemporaryFile() as input_file:
        input_file.write(raw)
        input_file.flush()
        output = subprocess.check_output([
            java, "-cp", f"{ORACLE}/.work/classes:{ORACLE}/.work/ShatteredPD-v4.0.1-Java.jar",
            "com.shatteredpixel.shatteredpixeldungeon.VaultDoorOracle", input_file.name,
        ])
    samples.append(json.loads(output))
fixture = dict(source="Official ShatteredPD-v4.0.1-Java.jar custom vault door selectors", samples=samples)
(ROOT / "crates/seedfinder-core/tests/fixtures/vault-doors.json").write_text(json.dumps(fixture, indent=2) + "\n")
