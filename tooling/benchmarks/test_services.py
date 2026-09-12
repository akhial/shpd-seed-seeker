"""Protocol/lifecycle checks with synthetic adapters; no engine timings."""
import importlib.util
import io
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import time
import types
import unittest
from unittest import mock

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent


def load(name):
    spec = importlib.util.spec_from_file_location(name, HERE / f'{name}.py')
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


services = load('effective_matches')
compare = load('compare_native')
READY = 'import sys, time, os\nprint(\'{"ready":true}\', flush=True)\n'


class ServiceTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)
        self.counter = 0
        self.addCleanup(self.tmp.cleanup)

    def launch(self, body):
        self.counter += 1
        service = services.Service(
            [sys.executable, '-u', '-c', READY + body],
            self.root / f'child-{self.counter}.log')
        # Save references before close detaches ownership, including on failure.
        process, log = service.process, service.log
        self.addCleanup(service.close, timeout=0.2, cleanup_timeout=1.0)
        return service, process, log

    def test_valid_request_and_idempotent_close(self):
        service, process, log = self.launch(
            'import json\n'
            'for line in sys.stdin:\n'
            ' doc=json.loads(line)\n'
            ' print(json.dumps({"tested":len(doc["seeds"]),"matches":[]}), flush=True)\n')
        self.assertEqual(service.request({'seeds': [13]})['tested'], 1)
        service.close()
        service.close()
        self.assertEqual(process.returncode, 0)
        self.assertTrue(process.stdout.closed and process.stdin.closed and log.closed)

    def test_trailing_records_partial_and_whitespace_are_rejected(self):
        for tail in ['{"extra":1}\n', '{"partial":', ' \n']:
            with self.subTest(tail=tail):
                service, process, log = self.launch(
                    f'sys.stdin.read()\nsys.stdout.write({tail!r})\nsys.stdout.flush()\n')
                with self.assertRaisesRegex(RuntimeError, 'trailing stdout'):
                    service.close()
                self.assertIsNotNone(process.returncode)
                self.assertTrue(log.closed)
                service.close()

    def test_same_write_and_prefetched_tail_are_checked(self):
        service, process, _ = self.launch(
            'sys.stdout.write(\'{"tested":1,"matches":[]}\\n{"extra":1}\\n\')\n'
            'sys.stdout.flush()\nsys.stdin.read()\n')
        self.assertEqual(service.request({'seeds': [13]})['tested'], 1)
        with self.assertRaisesRegex(RuntimeError, 'trailing stdout'):
            service.close()
        self.assertIsNotNone(process.returncode)
        # Deterministic decoded-buffer case: this stdout has no usable fileno.
        process = types.SimpleNamespace(
            stdin=io.StringIO(), stdout=io.StringIO('buffered\n'),
            returncode=0, wait=lambda **kw: 0, poll=lambda: 0)
        service = services.Service.__new__(services.Service)
        service.process, service.log, service._closed = process, None, False
        with self.assertRaisesRegex(RuntimeError, 'trailing stdout'):
            service.close()
        self.assertTrue(process.stdout.closed)

    def test_nonzero_exit_is_not_success(self):
        service, process, _ = self.launch('sys.stdin.read()\nsys.exit(7)\n')
        with self.assertRaisesRegex(RuntimeError, 'exit status 7'):
            service.close()
        self.assertEqual(process.returncode, 7)

    def test_large_trailing_output_does_not_block_shutdown(self):
        service, process, _ = self.launch(
            'sys.stdin.read()\nsys.stdout.write("x" * (4 * 1024 * 1024))\nsys.stdout.flush()\n')
        with self.assertRaisesRegex(RuntimeError, 'trailing stdout'):
            service.close(timeout=1.0, cleanup_timeout=1.0)
        self.assertIsNotNone(process.returncode)

    def test_timeout_reaps_child_even_when_stdout_already_closed(self):
        for close_stdout in [False, True]:
            with self.subTest(close_stdout=close_stdout):
                service, process, log = self.launch(
                    'sys.stdin.read()\n' + ('os.close(1)\n' if close_stdout else '')
                    + 'time.sleep(60)\n')
                began = time.monotonic()
                with self.assertRaisesRegex(RuntimeError, 'did not exit'):
                    service.close(timeout=0.05, cleanup_timeout=1.0)
                self.assertLess(time.monotonic() - began, 3.0)
                self.assertIsNotNone(process.returncode)
                self.assertTrue(log.closed)

    def test_constructor_failures_close_logs_and_reap_children(self):
        real_popen = subprocess.Popen
        for text in ['{"ready":true}', '{bad}\n', '{"ready":false}\n']:
            captured = []
            def capture(*args, **kwargs):
                process = real_popen(*args, **kwargs)
                captured.append((process, kwargs['stderr']))
                return process
            with self.subTest(text=text), mock.patch.object(services.subprocess, 'Popen', capture):
                with self.assertRaises((RuntimeError, json.JSONDecodeError)):
                    services.Service([sys.executable, '-u', '-c', f'print({text!r},end="",flush=True)'], self.root / 'bad-ready.log')
            process, log = captured[0]
            self.assertIsNotNone(process.returncode)
            self.assertTrue(log.closed and process.stdout.closed)
        opened = []
        def missing(*args, **kwargs):
            opened.append(kwargs['stderr'])
            raise FileNotFoundError('missing fixture executable')
        with mock.patch.object(services.subprocess, 'Popen', missing):
            with self.assertRaises(FileNotFoundError):
                services.Service(['missing-fixture'], self.root / 'missing.log')
        self.assertTrue(opened[0].closed)

    def test_response_without_final_newline_fails(self):
        service, _, _ = self.launch(
            'sys.stdin.readline()\nsys.stdout.write(\'{"tested":1,"matches":[]}\')\n'
            'sys.stdout.flush()\n')
        with self.assertRaisesRegex(RuntimeError, 'final newline'):
            service.request({'seeds': [13]})
        service.close()

    def test_cleanup_all_preserves_primary_failure(self):
        launched = [self.launch('sys.stdin.read()\nsys.exit(7)\n'),
                    self.launch('sys.stdin.read()\nprint("extra",flush=True)\n'),
                    self.launch('sys.stdin.read()\n')]
        with self.assertRaisesRegex(ValueError, 'primary request failure') as caught:
            try:
                raise ValueError('primary request failure')
            finally:
                services.close_services([item[0] for item in launched])
        self.assertIsInstance(caught.exception.__cause__, RuntimeError)
        self.assertTrue(all(process.poll() is not None and log.closed for _, process, log in launched))
        services.close_services([item[0] for item in launched])

    def test_open_pipe_writer_has_bounded_cleanup_without_cross_thread_close(self):
        read_fd, write_fd = os.pipe()
        process = types.SimpleNamespace(
            stdin=io.StringIO(), stdout=os.fdopen(read_fd, 'r'),
            returncode=0, wait=lambda **kw: 0, poll=lambda: 0)
        service = services.Service.__new__(services.Service)
        service.process, service.log, service._closed = process, None, False
        readers = []
        real_thread = services.threading.Thread
        def capture(*args, **kwargs):
            reader = real_thread(*args, **kwargs)
            readers.append(reader)
            return reader
        try:
            began = time.monotonic()
            with mock.patch.object(services.threading, 'Thread', capture):
                with self.assertRaisesRegex(RuntimeError, 'stdout did not reach EOF'):
                    service.close(timeout=0.05, cleanup_timeout=0.05)
            self.assertLess(time.monotonic() - began, 1.0)
            self.assertFalse(process.stdout.closed)
        finally:
            os.close(write_fd)
            for reader in readers:
                reader.join(timeout=2.0)
        self.assertTrue(all(not reader.is_alive() for reader in readers))
        self.assertTrue(process.stdout.closed)
        service.close()

    def test_request_failure_aborts_before_flushing_buffered_stdin(self):
        service, process, log = self.launch('time.sleep(60)\n')
        service.process.stdin.write('buffered request bytes')
        began = time.monotonic()
        with self.assertRaisesRegex(ValueError, 'interrupted request'):
            try:
                raise ValueError('interrupted request')
            finally:
                services.close_services([service])
        self.assertLess(time.monotonic() - began, 3.0)
        self.assertIsNotNone(process.returncode)
        self.assertTrue(process.stdin.closed and log.closed)

    def test_service_module_revision_is_required_before_launch(self):
        with mock.patch.object(services.subprocess, 'Popen') as popen:
            with self.assertRaisesRegex(ValueError, 'lifecycle revision'):
                compare.service_class(types.SimpleNamespace(Service=services.Service))
            popen.assert_not_called()
        self.assertIs(compare.service_class(services), services.Service)

    @unittest.skipUnless(os.name == 'posix', 'Executable script fixture uses a POSIX shebang')
    def test_comparison_does_not_publish_success_after_shutdown_failure(self):
        for variant in ['baseline', 'candidate']:
            directory = self.root / variant
            directory.mkdir()
            for name in ['seed-seeker', 'match_benchmark']:
                path = directory / name
                path.write_text('#!/usr/bin/env python3\n' + READY +
                    'import json\n'
                    'for line in sys.stdin:\n'
                    ' doc=json.loads(line)\n'
                    ' print(json.dumps({"tested":len(doc["seeds"]),"seconds":0.25,"matches":[]}),flush=True)\n'
                    'print("extra",flush=True)\n')
                path.chmod(0o755)
        output = self.root / 'result'
        result = subprocess.run([
            sys.executable, str(HERE / 'compare_native.py'),
            '--baseline-dir', str(self.root / 'baseline'),
            '--candidate-dir', str(self.root / 'candidate'),
            '--output', str(output), '--cases', 'cheap', '--workers', '1',
            '--reps', '1', '--seeds', '1'], text=True, capture_output=True, timeout=15)
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertTrue((output / 'failure.json').exists(), result.stderr)
        self.assertIn('trailing stdout', (output / 'failure.json').read_text())
        self.assertFalse((output / 'summary.json').exists())
        self.assertFalse((output / 'completion.json').exists())


if __name__ == '__main__':
    unittest.main()
