#!/usr/bin/env python3
"""Refresh the small floor corpus shared by the Rust region regression tests."""
import json
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tooling/parity"))
from compare_floors import item_id, simple, mob_name, NON_ORDINARY_MOBS

SOURCE = {
    "ghost_quest": "ghostreward", "wandmaker_quest": "wandmakerreward",
    "blacksmith_quest": "blacksmithreward", "imp_quest": "impreward",
    "sacrificial_prize": "sacrificialfire", "imp_shop_cache": "shop",
}


def item_row(record, vault_option=None):
    identity = item_id(record["simple_class"])
    if not record["searchable"] or record["kind"] == "artifact" or identity == "dart":
        return None
    source = record["source"]
    owner = simple(record["owner"])
    if source == "statue" and owner == "ArmoredStatue":
        source = "armored_statue"
    if source == "mimic" and owner in ("GoldenMimic", "CrystalMimic"):
        source = "golden_mimic" if owner == "GoldenMimic" else "crystal_mimic"
    secret = (record["room"] or "").startswith("Secret")
    if vault_option is not None:
        source, secret = "vaulttreasure", False
    effect = simple(record["enchantment"] or record["glyph"]) or "-"
    effect = {"AntiMagic": "Anti-Magic", "AntiEntropy": "Anti-Entropy"}.get(effect, effect)
    return [identity, record["search_upgrade"], record["cursed"], effect,
            SOURCE.get(source, source.replace("_", "")), secret]


def main():
    jobs = [(seed, 0) for seed in (
        "AAA-AAA-AAA", "AAA-AAA-AAB", "AAA-AAA-AAC", "ABC-DEF-GHI",
        "ZZZ-ZZZ-ZZZ", "CVB-VKT-LUY", "AAA-AAA-AIC", "AAA-AAA-AFG", "AAA-AAA-AAR",
    )]
    jobs += [("AAA-AAA-AAF", mask) for mask in (0, 8, 32, 64, 104)]
    rows = []
    for seed, mask in jobs:
        document = json.loads(subprocess.check_output([
            str(ROOT / "tooling/oracle-4.0/run.sh"), "--seed", seed,
            "--floors", "1-24", "--vault", "--challenges", str(mask), "--format", "json",
        ]))
        records = document["records"]
        for level in (r for r in records if r["record"] == "level" and r["branch"] == 0):
            items = [item_row(r) for r in records if r["record"] == "item"
                     and r["branch"] == 0 and r["depth"] == level["depth"]]
            vault = sorted((r for r in records if r["record"] == "item" and r["branch"] == 1
                            and r["depth"] == level["depth"] and r["room"] != "VaultFinalRoom"
                            and r["searchable"] and r["simple_class"] != "Dart"),
                           key=lambda r: (r["cell"], r["choice"]))
            items += [item_row(r, option) for option, r in enumerate(vault, 6)]
            ignored = {m.lower() for m in NON_ORDINARY_MOBS}
            ordinary = [[mob_name(m["class"]), m["cell"]] for m in level["mobs"]
                        if mob_name(m["class"]) not in ignored
                        and not (mob_name(m["class"]) == "skeleton" and m["room"] == "MassGraveRoom")]
            rows.append(dict(seed=seed, challenges=mask, depth=level["depth"],
                             size=[level["width"], level["height"]], mapHash=level["map_hash"],
                             entrance=level["entrance"], exit=level["exit"], feeling=level["feeling"],
                             occupied=sorted(m["cell"] for m in level["mobs"]),
                             mobs=sorted(ordinary), items=sorted(i for i in items if i)))
        print(seed, mask, flush=True)
    fixture = ROOT / "crates/seedfinder-core/tests/fixtures/main-floors-v4.0.2.json"
    fixture.write_text('{\n  "source": ' + json.dumps(document["records"][0]["game_jar_sha256"])
                       + ',\n  "floors": [\n'
                       + ',\n'.join('    ' + json.dumps(row, separators=(",", ":")) for row in rows)
                       + '\n  ]\n}\n')


if __name__ == "__main__":
    main()
