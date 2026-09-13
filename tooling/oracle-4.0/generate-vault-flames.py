#!/usr/bin/env python3
"""Capture seeded vent schedules from the pinned official v4.0 JAR."""
import json
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[2]
ORACLE = ROOT / 'tooling/oracle-4.0'
subprocess.run([str(ORACLE / 'build.sh')], check=True, stdout=subprocess.DEVNULL)
samples = []
for seed in ('AAA-AAA-AAA', 'FOI-QDX-EMJ', 'BAD-RAT-KNG', 'HEL-LOO-WRD'):
    data = json.loads(subprocess.check_output([
        'java', '-cp', f'{ORACLE}/.work/classes:{ORACLE}/.work/ShatteredPD-v4.0.0-Java.jar',
        'com.shatteredpixel.shatteredpixeldungeon.ParityOracle', '--seed', seed,
        '--floors', '17-19', '--map-contents', '--acquire-hourglass', '--vault', '--format', 'json']))
    level = next(r for r in data['records'] if r.get('record') == 'level' and r['branch'] == 1)
    families = sorted({r['class'].rsplit('.', 1)[-1] for r in level['rooms']
                       if 'Fire' in r['class'] or 'Flame' in r['class']})
    cycles = [[c[k] for k in ('cell', 'initialCooldown', 'cooldown', 'triggers')]
              for c in level['flame_cycles']]
    samples.append(dict(seed=seed, depth=level['depth'], families=families, cycles=cycles))
fixture = dict(source='Official ShatteredPD-v4.0.0-Java.jar: VaultFlameTraps setup arrays; cycles are [cell, initialCooldown, cooldown, triggers].', samples=samples)
(ROOT / 'crates/seedfinder-core/tests/fixtures/vault-flames.json').write_text(json.dumps(fixture, indent=2)+'\n')
print([(s['seed'], s['families'], len(s['cycles'])) for s in samples])
