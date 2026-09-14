#!/usr/bin/env python3
"""Regenerate generation-time map fixtures from the pinned official Java JAR."""
import concurrent.futures
import json
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[2]
ORACLE = ROOT / 'tooling/oracle-4.0'
JAVA = ['java', '-cp', f'{ORACLE}/.work/classes:{ORACLE}/.work/ShatteredPD-v4.0.0-Java.jar']
MAIN = 'com.shatteredpixel.shatteredpixeldungeon.'

def format_fixture(fixture):
    """Keep sample metadata readable and each heap/actor/effect record on one line."""
    lines = ['{', '  "source": '+json.dumps(fixture['source'])+',', '  "samples": [']
    for i, record in enumerate(fixture['samples']):
        lines.append('    {')
        for j, (key, value) in enumerate(record.items()):
            prefix = '      '+json.dumps(key)+': '
            comma = ',' if j < len(record)-1 else ''
            if isinstance(value, list) and value:
                lines.append(prefix+'[')
                lines += ['        '+json.dumps(entry, separators=(',', ':'))
                          +(',' if k < len(value)-1 else '') for k, entry in enumerate(value)]
                lines.append('      ]'+comma)
            else:
                lines.append(prefix+json.dumps(value)+comma)
        lines.append('    }'+(',' if i < len(fixture['samples'])-1 else ''))
    lines += ['  ]', '}']
    return '\n'.join(lines)+'\n'

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

def main():
    subprocess.run([str(ORACLE / 'build.sh')], check=True, stdout=subprocess.DEVNULL)
    jobs = [(seed,0,None,None) for seed in ('AAA-AAA-AAA','BAD-RAT-KNG','HEL-LOO-WRD','ZZZ-ZZZ-ZZZ')]
    jobs += [('AAA-AAA-AAA',111,None,None), ('AAA-AAA-AAA',0,'mimic_tooth',None)]
    jobs += [(seed,challenge,None,(depth,variant)) for seed,depth,variant in [('AAA-AAA-AAA',13,1),('ZZZ-ZZZ-ZZZ',12,2)] for challenge in (0,32)]
    with concurrent.futures.ThreadPoolExecutor(max_workers=4) as pool:
        samples = [s for batch in pool.map(generate,jobs) for s in batch]
    fixture = dict(source='official ShatteredPD-v4.0.0-Java.jar; ParityOracle --map-contents --acquire-hourglass', samples=samples)
    (ROOT/'crates/seedfinder-core/tests/fixtures/map-contents.json').write_text(format_fixture(fixture))
    print(f'Wrote {len(samples)} samples')

if __name__ == '__main__':
    main()
