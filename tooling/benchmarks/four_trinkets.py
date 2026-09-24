#!/usr/bin/env python3
"""Warmed, alternating Java/native benchmark for the four-trinket garden query."""
import argparse
from concurrent.futures import ThreadPoolExecutor
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import platform
import subprocess
import time

from effective_matches import JAR, JAR_SHA256, Service, digest, seeds_at

QUERY = {
    'auto_apply_trinket': True,
    'exclude_blacksmith_rewards': True,
    'floor_requirements': [
        {'depth': 4, 'feeling': 'grass'},
        {'any_rooms': ['garden', 'secret_garden'], 'depth': 7, 'feeling': 'dark'}],
    'max_depth': 22,
    'requirements': [
        {'item': 'dimensional_sundial', 'kind': 'trinket'},
        {'item': 'rat_skull', 'kind': 'trinket'},
        {'item': 'parchment_scrap', 'kind': 'trinket'},
        {'item': 'petrified_seed', 'kind': 'trinket'}],
}
POSITIVE = [1118249074502, 4356999994986, 1469088515621]


def java_request(pool, services, seeds):
    replies = list(pool.map(lambda pair: pair[1].request(
        {'seeds': seeds[pair[0]::len(services)]}), enumerate(services)))
    return {'tested': sum(r['tested'] for r in replies),
            'matches': [m for r in replies for m in r['matches']]}


def run(args):
    args.output.mkdir(parents=True, exist_ok=False)
    assert digest(JAR) == JAR_SHA256
    binary = args.binary.resolve()
    services = []
    environment = {
        'started_utc': datetime.now(timezone.utc).isoformat(),
        'platform': platform.platform(), 'cpus': os.cpu_count(),
        'load': os.getloadavg(), 'rustc': subprocess.check_output(['rustc', '-Vv'], text=True),
        'java': subprocess.check_output([args.java, '-version'], stderr=subprocess.STDOUT, text=True),
        'commit': subprocess.check_output(['git', 'rev-parse', 'HEAD'], text=True).strip(),
        'profile_sha256': digest('target/benchmark/four-trinkets.profdata'),
        'binary_sha256': digest(binary), 'jar_sha256': digest(JAR),
        'query': QUERY, 'parameters': vars(args) | {'binary': str(binary), 'output': str(args.output)},
        'sources': {str(p): digest(p) for p in [Path(__file__),
            Path('crates/seedfinder-ffi/examples/match_benchmark.rs'),
            Path('tooling/java-finder/src/com/shatteredpixel/shatteredpixeldungeon/JarSeedFinder.java')]},
    }
    (args.output / 'environment.json').write_text(json.dumps(environment, indent=2) + '\n')
    try:
        native = Service([str(binary), json.dumps(QUERY, separators=(',', ':')), str(args.workers)],
                         args.output / 'native.log')
        services.append(native)
        assert native.ready['preferred'] == []  # Explicit trinket requirements disable auto choice.
        java_command = [args.java, '-Xms128m', '-Xmx512m', '-XX:ActiveProcessorCount=1',
            '-cp', f'tooling/java-finder/.work/classes{os.pathsep}{JAR}',
            'com.shatteredpixel.shatteredpixeldungeon.JarSeedFinder', '--stream',
            '--four-trinket-garden', '--warmup', '0']
        java = []
        for i in range(args.workers):
            service = Service(java_command, args.output / f'java-{i}.log')
            java.append(service)
            services.append(service)
        with ThreadPoolExecutor(max_workers=args.workers) as pool:
            warm = seeds_at(8_000_000, args.warmup * args.workers) + POSITIVE
            java_request(pool, java, warm)
            native.request({'seeds': warm})
            print(f'warmed; {args.minutes:g} minutes per mode', flush=True)
            elapsed = {'java': 0.0, 'native': 0.0}
            offsets = {'java': 0, 'native': 0}
            counts = {'java': 0, 'native': 0}
            hits = {'java': 0, 'native': 0}
            block = 0
            with (args.output / 'blocks.jsonl').open('w') as raw:
                while min(elapsed.values()) < args.minutes * 60:
                    active = [mode for mode in ('java', 'native')
                              if elapsed[mode] < args.minutes * 60]
                    if block % 2:
                        active.reverse()
                    for mode in active:
                        count = args.batch if mode == 'java' else args.batch * 4
                        start = args.start + offsets[mode]
                        seeds = seeds_at(start, count)
                        began = time.perf_counter()
                        result = (java_request(pool, java, seeds) if mode == 'java'
                                  else native.request({'seeds': seeds}))
                        seconds = time.perf_counter() - began
                        assert result['tested'] == count
                        result.update(mode=mode, block=block, start_index=start, seconds=seconds)
                        raw.write(json.dumps(result, separators=(',', ':')) + '\n')
                        raw.flush()
                        elapsed[mode] += seconds
                        offsets[mode] += count
                        counts[mode] += count
                        hits[mode] += len(result['matches'])
                    if block % 256 == 0 or not active:
                        print(f'block {block}: ' + ', '.join(
                            f'{m}={elapsed[m]:.1f}s/{counts[m]} seeds/{hits[m]} matches'
                            for m in ('java', 'native')), flush=True)
                    block += 1
            summary = {m: {'seeds': counts[m], 'matches': hits[m], 'seconds': elapsed[m],
                           'seeds_per_second': counts[m] / elapsed[m],
                           'matches_per_minute': 60 * hits[m] / elapsed[m]}
                       for m in ('java', 'native')}
            summary.update(native_over_java_matches_per_minute=(
                summary['native']['matches_per_minute'] / summary['java']['matches_per_minute']
                if hits['java'] else None), equivalence_tests='skipped',
                known_positive_count=len(POSITIVE))
            (args.output / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
            environment['finished_utc'] = datetime.now(timezone.utc).isoformat()
            (args.output / 'environment.json').write_text(json.dumps(environment, indent=2) + '\n')
            print(json.dumps(summary, indent=2), flush=True)
    finally:
        for service in services:
            service.close()


def train(args):
    args.output.mkdir(parents=True, exist_ok=True)
    service = Service([str(args.binary.resolve()), json.dumps(QUERY, separators=(',', ':')), '1'],
                      args.output / 'training.log')
    try:
        service.request({'seeds': seeds_at(9_000_000, 16_384) + POSITIVE})
    finally:
        service.close()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, default=Path('target/benchmark/release/examples/match_benchmark'))
    parser.add_argument('--output', type=Path, default=Path('target/benchmark') /
                        ('four-trinkets-' + datetime.now().strftime('%Y%m%d-%H%M%S')))
    parser.add_argument('--java', default='java')
    parser.add_argument('--workers', type=int, default=8)
    parser.add_argument('--minutes', type=float, default=10)
    parser.add_argument('--warmup', type=int, default=1024)
    parser.add_argument('--batch', type=int, default=3072)
    parser.add_argument('--start', type=int, default=1_000_000)
    parser.add_argument('--train', action='store_true')
    args = parser.parse_args()
    if min(args.workers, args.minutes, args.batch) <= 0 or min(args.warmup, args.start) < 0:
        parser.error('positive workers/minutes/batch and nonnegative warmup/start required')
    (train if args.train else run)(args)


if __name__ == '__main__':
    main()
