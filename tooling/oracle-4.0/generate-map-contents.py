#!/usr/bin/env python3
"""Regenerate generation-time map fixtures from the pinned official Java JAR."""
import concurrent.futures
import json
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[2]
ORACLE = ROOT / 'tooling/oracle-4.0'
subprocess.run([str(ORACLE / 'build.sh')], check=True, stdout=subprocess.DEVNULL)
JAVA = ['java', '-cp', f'{ORACLE}/.work/classes:{ORACLE}/.work/ShatteredPD-v4.0.0-Java.jar']
MAIN = 'com.shatteredpixel.shatteredpixeldungeon.'

def sample(record, seed, challenge, trinket):
    mobs = []
    for mob in record['mobs']:
        kind = mob['class'].rsplit('.', 1)[-1]
        if kind in ('CrystalGuardian', 'CrystalSpire', 'CrystalWisp'):
            kind = mob['sprite_class'].rsplit('$', 1)[-1] + kind
        mobs.append(dict(cell=mob['cell'], kind=kind, sleeping=mob['sleeping'], items=mob['items']))
    return dict(seed=seed, depth=record['depth'], branch=record['branch'], challenges=challenge,
                trinket=trinket, width=record['width'], height=record['height'], terrainHash=record['map_hash'],
                heaps=record['heaps'], mobs=mobs, plants=record['plants'], effects=record['effects'], traps=record['traps'])

def generate(job):
    seed, challenge, trinket, mine = job
    command = JAVA[:]
    if trinket:
        command.insert(1, '-Dseedfinder.trinket=MimicTooth')
    if mine:
        depth, variant = mine
        command += [MAIN+'MiningMapOracle', seed, str(depth), str(variant), str(challenge), 'contents']
    else:
        command += [MAIN+'ParityOracle', '--seed', seed, '--floors', '1-24', '--map-contents',
                    '--acquire-hourglass', '--format', 'json', '--challenges', str(challenge), '--vault']
    document = json.loads(subprocess.check_output(command))
    records = [document] if mine else [r for r in document['records'] if r['record']=='level' and r['depth'] not in (10,20,25)]
    return [sample(r,seed,challenge,trinket) for r in records]

jobs = [(seed,0,None,None) for seed in ('AAA-AAA-AAA','BAD-RAT-KNG','HEL-LOO-WRD','ZZZ-ZZZ-ZZZ')]
jobs += [('AAA-AAA-AAA',111,None,None), ('AAA-AAA-AAA',0,'mimic_tooth',None)]
jobs += [(seed,challenge,None,(depth,variant)) for seed,depth,variant in [('AAA-AAA-AAA',13,1),('ZZZ-ZZZ-ZZZ',12,2)] for challenge in (0,32)]
with concurrent.futures.ThreadPoolExecutor(max_workers=4) as pool:
    samples = [s for batch in pool.map(generate,jobs) for s in batch]
fixture = dict(source='official ShatteredPD-v4.0.0-Java.jar; ParityOracle --map-contents --acquire-hourglass', samples=samples)
(ROOT/'crates/seedfinder-core/tests/fixtures/map-contents.json').write_text(json.dumps(fixture,indent=2)+'\n')
print(f'Wrote {len(samples)} samples')
