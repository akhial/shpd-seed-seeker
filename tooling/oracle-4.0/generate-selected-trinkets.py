#!/usr/bin/env python3
"""Refresh the small selected-trinket loot suite from the official pinned JAR."""
import json
import os
from pathlib import Path
import re
import subprocess

ROOT = Path(__file__).resolve().parents[2]
ORACLE = ROOT / "tooling/oracle-4.0"
FIXTURE = ROOT / "crates/seedfinder-core/tests/fixtures/selected_trinkets_v4.0.1.json"


def stable_id(name):
    result = re.sub(r"(?<!^)(?=[A-Z])", "_", name).lower()
    return result.replace("wand_of_", "wand_").replace("ring_of_", "ring_")


def main():
    cases = json.loads(FIXTURE.read_text())
    for case in cases:
        class_name = "".join(part.title() for part in case["trinket"].split("_"))
        env = os.environ.copy()
        env["JAVA_TOOL_OPTIONS"] = f"-Dseedfinder.trinket={class_name}"
        document = json.loads(subprocess.check_output([
            str(ORACLE / "run.sh"), "--seed", case["seed"], "--floors", "1-24",
            "--vault", "--format", "json",
        ], env=env))
        items = []
        for record in document["records"]:
            if record["record"] != "item" or not record["searchable"]:
                continue
            if record["source"] == "imp_quest" or record["kind"] == "artifact":
                continue
            identity = stable_id(record["simple_class"])
            if identity == "dart":
                continue
            effect = record["enchantment"] or record["glyph"]
            effect = effect.rsplit(".", 1)[-1] if effect else "-"
            effect = {"AntiMagic": "Anti-Magic", "AntiEntropy": "Anti-Entropy"}.get(effect, effect)
            items.append([record["depth"], identity, record["search_upgrade"], record["cursed"], effect])
        case["items"] = sorted(items)
        print(case["seed"], case["trinket"], len(items), flush=True)
    FIXTURE.write_text("[\n" + ",\n".join(json.dumps(case, separators=(",", ":")) for case in cases) + "\n]\n")


if __name__ == "__main__":
    main()
