#!/usr/bin/env python3
"""Ten warmed minutes per mode/query; raw evidence stays in an untracked output directory."""
import argparse
from collections import Counter
from concurrent.futures import ThreadPoolExecutor
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import platform
import random
import subprocess
import time

BINARY = Path('target/benchmark/aarch64-apple-darwin/release/examples/match_benchmark')
JAR = Path('tooling/java-finder/.work/ShatteredPD-v4.0.0-Java.jar')
JAR_SHA256 = 'b3e6f9508dea1a7a32a9934e2bc18f20a9a905df5732550404294340d31c87a1'
MODES = ('java', 'native_off', 'native_auto')
CASES = {
    'blade_might': ('RunicBlade,RingOfMight', {
        'max_depth': 19, 'requirements': [
            {'item': 'runic_blade', 'upgrade': 2,
             'effect': ['Grim', 'Corrupting', 'Vampiric', 'Crystal']},
            {'item': 'ring_might', 'upgrade': 2}]}),
    'crossbow': ('Crossbow', {
        'max_depth': 19, 'requirements': [{'item': 'crossbow', 'upgrade': 5}]}),
}


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def seeds_at(start, count):
    return [(i * 3_355_211_884_971 + 812_345_678_901) % (26 ** 9)
            for i in range(start, start + count)]


class Service:
    def __init__(self, command, log):
        self.log = log.open('w')
        self.process = subprocess.Popen(command, stdin=subprocess.PIPE,
            stdout=subprocess.PIPE, stderr=self.log, text=True)
        self.ready = self.receive()
        assert self.ready['ready']

    def receive(self):
        line = self.process.stdout.readline()
        if not line:
            raise RuntimeError(f'adapter exited: see {self.log.name}')
        return json.loads(line)

    def request(self, document):
        self.process.stdin.write(json.dumps(document, separators=(',', ':')) + '\n')
        self.process.stdin.flush()
        response = self.receive()
        assert response['tested'] == len(document['seeds'])
        seeds = [row['seed'] for row in response['matches']]
        assert len(set(seeds)) == len(seeds) and set(seeds) <= set(document['seeds'])
        return response

    def close(self):
        self.process.stdin.close()
        try:
            self.process.wait(timeout=10)
        except subprocess.TimeoutExpired:
            self.process.kill()
            self.process.wait()
        self.process.stdout.close()
        self.log.close()


def java_request(pool, services, seeds, recipes=None):
    def request(i, service):
        doc = {'seeds': seeds[i::len(services)]}
        if recipes is not None:
            doc.update(verify=True, trinkets=[
                ''.join(w.capitalize() for w in r['trinket'].split('_')) if r.get('trinket') else None
                for r in recipes[i::len(services)]])
        return service.request(doc)
    replies = list(pool.map(lambda pair: request(*pair), enumerate(services)))
    return {'tested': sum(r['tested'] for r in replies),
            'matches': [row for r in replies for row in r['matches']]}


def summarize(blocks):
    rows = {m: [b for b in blocks if b['mode'] == m] for m in MODES}
    def rate(sample):
        return 60 * sum(len(b['matches']) for b in sample) / sum(b['seconds'] for b in sample)
    summary = {m: {'seeds': sum(b['tested'] for b in rows[m]),
                   'matches': sum(len(b['matches']) for b in rows[m]),
                   'seconds': sum(b['seconds'] for b in rows[m]),
                   'matches_per_minute': rate(rows[m])} for m in MODES}
    rng = random.Random(20260911)
    draws = {m: [] for m in MODES}
    ratios = {'auto_over_off': [], 'auto_over_java': []}
    # Pair native AB/BA blocks on identical seeds. Java searches a shorter
    # prefix in its ten minutes; independently resample its adjacent blocks.
    groups = {m: [rows[m][i:i + 2] for i in range(0, len(rows[m]), 2)] for m in MODES}
    for _ in range(5000):
        indices = rng.choices(range(len(groups['native_off'])), k=len(groups['native_off']))
        rates = {m: rate([b for i in indices for b in groups[m][i]]) for m in MODES[1:]}
        rates['java'] = rate([b for group in rng.choices(groups['java'], k=len(groups['java'])) for b in group])
        for m in MODES:
            draws[m].append(rates[m])
        if rates['native_off'] and rates['java']:
            ratios['auto_over_off'].append(rates['native_auto'] / rates['native_off'])
            ratios['auto_over_java'].append(rates['native_auto'] / rates['java'])
    def interval(values):
        values.sort()
        return [values[int((len(values) - 1) * p)] for p in (0.025, 0.975)] if values else None
    for m in MODES:
        summary[m]['matches_per_minute_95_ci'] = interval(draws[m])
    for name, base in [('auto_over_off', 'native_off'), ('auto_over_java', 'java')]:
        summary[name] = summary['native_auto']['matches_per_minute'] / summary[base]['matches_per_minute'] if summary[base]['matches'] else None
        summary[name + '_95_ci'] = interval(ratios[name])
    off, auto = ({r['seed'] for b in rows[m] for r in b['matches']} for m in MODES[1:])
    summary.update(gained=len(auto - off), lost=len(off - auto))
    summary['automatic_choices_on_matches'] = dict(Counter(
        r.get('trinket') or 'none' for b in rows['native_auto'] for r in b['matches']))
    return summary


def run_case(args, name, java_class, query):
    services, blocks = [], []
    try:
        native = {}
        for mode in MODES[1:]:
            doc = dict(query, auto_apply_trinket=mode == 'native_auto')
            service = Service([str(args.binary), json.dumps(doc), str(args.workers)],
                              args.output / f'{name}-{mode}.log')
            services.append(service)
            native[mode] = service
        if name == 'blade_might':
            assert native['native_off'].ready['link'] == 'QyAhKCsAAeAAAuoKAA'
        if name == 'crossbow':
            assert native['native_auto'].ready['preferred'] == []
        if args.train:
            for service in native.values():
                service.request({'seeds': seeds_at(9_000_000, 4096)})
            return
        requirement = query['requirements'][0]
        command = [args.java, '-Xms128m', '-Xmx512m', '-XX:ActiveProcessorCount=1',
            '-cp', f'tooling/java-finder/.work/classes{os.pathsep}{JAR}',
            'com.shatteredpixel.shatteredpixeldungeon.JarSeedFinder', '--stream',
            '--item', java_class, '--upgrade', str(requirement['upgrade']),
            '--floors', '19', '--skip-boss-floors', '--warmup', '0']
        if 'effect' in requirement:
            command += ['--effect', ','.join(requirement['effect'])]
        if name == 'crossbow':
            command += ['--no-vault']
        java = []
        for i in range(args.workers):
            service = Service(command, args.output / f'{name}-java-{i}.log')
            services.append(service)
            java.append(service)
        with ThreadPoolExecutor(max_workers=args.workers) as pool:
            # Regression: two mutually exclusive vault prizes are not a match.
            warm = seeds_at(8_000_000, args.warmup * args.workers) + [447_165_152_125]
            java_warm = java_request(pool, java, warm)
            native_warm = native['native_off'].request({'seeds': warm})
            assert {r['seed'] for r in java_warm['matches']} == {r['seed'] for r in native_warm['matches']}
            native['native_auto'].request({'seeds': warm})
            print(f'{name}: warm-up complete; {args.minutes:g} minutes per mode', flush=True)
            elapsed = dict.fromkeys(MODES, 0.0)
            offsets = dict.fromkeys(MODES, 0)
            block = 0
            with (args.output / f'{name}.jsonl').open('w') as raw:
                while min(elapsed.values()) < args.minutes * 60:
                    active = ([] if elapsed['java'] >= args.minutes * 60 else ['java'])
                    if min(elapsed[m] for m in MODES[1:]) < args.minutes * 60:
                        active += list(MODES[1:])
                    # Reverse order each round; only one mode occupies the CPU.
                    if block % 2:
                        active.reverse()
                    for mode in active:
                        count = args.batch if mode == 'java' else args.batch * 4
                        seeds = seeds_at(args.start + offsets[mode], count)
                        began = time.perf_counter()
                        result = (java_request(pool, java, seeds) if mode == 'java'
                                  else native[mode].request({'seeds': seeds}))
                        seconds = time.perf_counter() - began
                        result.update(mode=mode, block=block, start_index=args.start + offsets[mode], seconds=seconds)
                        blocks.append(result)
                        raw.write(json.dumps(result, separators=(',', ':')) + '\n')
                        raw.flush()
                        elapsed[mode] += seconds
                        offsets[mode] += count
                    print(f'{name}: seconds ' + ' / '.join(f'{m}={elapsed[m]:.1f}' for m in MODES), flush=True)
                    block += 1
            # Untimed full-query replay of every native recipe, then independent
            # full-depth JAR replay of every reported supporting item tuple.
            verified = Counter()
            with (args.output / f'{name}-verification.jsonl').open('w') as replay_log:
                for mode in MODES:
                    recipes = [r for b in blocks if b['mode'] == mode for r in b['matches']]
                    for offset in range(0, len(recipes), args.batch):
                        batch = recipes[offset:offset + args.batch]
                        seeds = [r['seed'] for r in batch]
                        if mode != 'java':
                            replay = native[mode].request({'seeds': seeds, 'verify': True,
                                                          'trinkets': [r['trinket'] for r in batch]})
                            assert all(r['verified'] for r in replay['matches'])
                        replay = java_request(pool, java, seeds, batch)
                        actual = {r['seed']: Counter(map(tuple, r['witnesses'])) for r in replay['matches']}
                        for r in batch:
                            wanted = Counter(map(tuple, r['witnesses']))
                            assert wanted and not (wanted - actual[r['seed']]), (name, mode, r, actual[r['seed']])
                            verified[mode] += 1
                        replay_log.write(json.dumps({'mode': mode, 'verified': len(batch), 'seeds': seeds}) + '\n')
                    print(f'{name}: replayed {mode} {verified[mode]} matches', flush=True)
            # Full Java sample, including its negatives, must match native off.
            for b in (b for b in blocks if b['mode'] == 'java'):
                replay = native['native_off'].request({'seeds': seeds_at(b['start_index'], b['tested'])})
                assert {r['seed'] for r in replay['matches']} == {r['seed'] for r in b['matches']}, (name, b['block'], 'Java/native parity')
            summary = summarize(blocks)
            summary.update(query=query, verified=dict(verified), java_native_parity=True)
            for m in MODES:
                assert verified[m] == summary[m]['matches']
            if name == 'crossbow':
                assert summary['gained'] == summary['lost'] == 0
                assert set(summary['automatic_choices_on_matches']) <= {'none'}
            (args.output / f'{name}-summary.json').write_text(json.dumps(summary, indent=2) + '\n')
            print(name, json.dumps(summary), flush=True)
            return summary
    finally:
        for service in services:
            service.close()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path)
    parser.add_argument('--binary', type=Path, default=BINARY)
    parser.add_argument('--java', default='java')
    parser.add_argument('--workers', type=int, default=12)
    parser.add_argument('--minutes', type=float, default=10)
    parser.add_argument('--warmup', type=int, default=1024, help='untimed seeds per worker/mode')
    parser.add_argument('--batch', type=int, default=3072, help='Java seeds per block; native uses four times this')
    parser.add_argument('--start', type=int, default=1_000_000)
    parser.add_argument('--train', action='store_true')
    args = parser.parse_args()
    if min(args.workers, args.minutes, args.batch) <= 0 or min(args.warmup, args.start) < 0:
        parser.error('positive workers/minutes/batch and nonnegative warmup/start required')
    args.output = args.output or Path('target/benchmark') / datetime.now().strftime('run-%Y%m%d-%H%M%S')
    args.output.mkdir(parents=True, exist_ok=False)
    assert args.train or digest(JAR) == JAR_SHA256
    sources = [Path(__file__), Path('tooling/benchmarks/run.sh'),
               Path('crates/seedfinder-ffi/examples/match_benchmark.rs'),
               *Path('tooling/java-finder/src').rglob('*.java'),
               Path('tooling/java-finder/build.sh'), Path('tooling/java-finder/run.sh')]
    environment = {'started_utc': datetime.now(timezone.utc).isoformat(),
        'platform': platform.platform(), 'cpus': os.cpu_count(), 'load': os.getloadavg(),
        'rustc': subprocess.check_output(['rustc', '-Vv'], text=True),
        'java': subprocess.check_output([args.java, '-version'], stderr=subprocess.STDOUT, text=True),
        'commit': subprocess.check_output(['git', 'rev-parse', 'HEAD'], text=True).strip(),
        'profile_sha256': digest('target/benchmark/benchmark.profdata') if not args.train else None,
        'engine_source_sha256': hashlib.sha256(b''.join(str(p).encode() + p.read_bytes()
            for p in sorted(Path('crates/seedfinder-core/src').rglob('*.rs')))).hexdigest(),
        'binary_sha256': digest(args.binary), 'jar_sha256': digest(JAR) if JAR.exists() else None,
        'sources': {str(p): digest(p) for p in sources},
        'parameters': {k: str(v) if isinstance(v, Path) else v for k, v in vars(args).items()}}
    (args.output / 'environment.json').write_text(json.dumps(environment, indent=2) + '\n')
    summaries = {name: run_case(args, name, cls, query) for name, (cls, query) in CASES.items()}
    (args.output / 'summary.json').write_text(json.dumps(summaries, indent=2) + '\n')
    environment['finished_utc'] = datetime.now(timezone.utc).isoformat()
    (args.output / 'environment.json').write_text(json.dumps(environment, indent=2) + '\n')


if __name__ == '__main__':
    main()
