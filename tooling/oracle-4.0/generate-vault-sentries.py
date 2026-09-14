#!/usr/bin/env python3
"""Capture sentry setup and each ray/cone's exact cell coverage from v4.0."""
import json
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[2]
ORACLE = ROOT / 'tooling/oracle-4.0'
subprocess.run([str(ORACLE / 'build.sh')], check=True, stdout=subprocess.DEVNULL)
samples = []
for seed in ('AAA-AAA-AAA', 'FOI-QDX-EMJ', 'BAD-RAT-KNG', 'HEL-LOO-WRD', 'GSA-DGS-ADG', 'SAD-BAD-RAD', 'AAA-AAA-AAB', 'AAA-AAA-AAC', 'AAA-AAA-AAD'):
    data = json.loads(subprocess.check_output([
        'java', '-cp', f'{ORACLE}/.work/classes:{ORACLE}/.work/ShatteredPD-v4.0.0-Java.jar',
        'com.shatteredpixel.shatteredpixeldungeon.ParityOracle', '--seed', seed,
        '--floors', '17-19', '--map-contents', '--acquire-hourglass', '--vault', '--format', 'json']))
    level = next(r for r in data['records'] if r.get('record') == 'level' and r['branch'] == 1)
    samples.append(dict(seed=seed, depth=level['depth'], sentries=level['sentries']))
fixture = dict(source='Official ShatteredPD-v4.0.0-Java.jar: VaultLaser/VaultSentry setup; Ballistica, ConeAOE and Level.updateFieldOfView coverage before play.', samples=samples)
# Keep each sentry on one line so directions/coverage are easy to compare.
lines = ['{', '  "source": '+json.dumps(fixture['source'])+',', '  "samples": [']
for i, sample in enumerate(samples):
    lines += ['    {', '      "seed": '+json.dumps(sample['seed'])+',', '      "depth": '+str(sample['depth'])+',', '      "sentries": [']
    lines += ['        '+json.dumps(sentry, separators=(',', ':'))+(',' if j<len(sample['sentries'])-1 else '') for j,sentry in enumerate(sample['sentries'])]
    lines += ['      ]', '    }'+(',' if i<len(samples)-1 else '')]
lines += ['  ]', '}']
(ROOT / 'crates/seedfinder-core/tests/fixtures/vault-sentries.json').write_text('\n'.join(lines)+'\n')
print([(s['seed'], s['depth'], len(s['sentries'])) for s in samples])
print('Scan shapes:', sorted({tuple(r['pattern']['scan']) for s in samples for r in s['sentries'] if 'scan' in r['pattern']}))
