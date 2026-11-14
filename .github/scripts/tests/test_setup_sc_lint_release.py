"""Execute the release install shell with controlled archives in the published layout.

Real 0.8.9 Linux/Windows release archives were inspected separately. These
fixtures verify path discovery and failure behavior without network access.
"""
import hashlib
import io
import json
import os
import shutil
from pathlib import Path
import subprocess
import tarfile
import tempfile
import textwrap
import unittest

ROOT = Path(__file__).resolve().parents[3]
ACTION = ROOT / '.github/actions/setup-sc-lint/action.yml'
BINARIES = ('sc-lint', 'sc-lint-boundary', 'sc-lint-portability', 'sc-lint-runtime')


class ReleaseSetupTests(unittest.TestCase):
    def run_install(self, directory, *, missing=None, corrupt_checksum=False):
        root = Path(directory)
        assets = root / 'assets'
        assets.mkdir()
        archive_name = 'sc-lint_0.8.9_x86_64-unknown-linux-gnu.tar.gz'
        prefix = archive_name.removesuffix('.tar.gz')
        archive = assets / archive_name
        with tarfile.open(archive, 'w:gz') as output:
            for binary in BINARIES:
                if binary == missing:
                    continue
                data = b'#!/bin/sh\nexit 0\n'
                entry = tarfile.TarInfo(f'{prefix}/bin/{binary}')
                entry.size = len(data)
                entry.mode = 0o755
                output.addfile(entry, io.BytesIO(data))
        digest = '0' * 64 if corrupt_checksum else hashlib.sha256(archive.read_bytes()).hexdigest()
        (assets / 'checksums.txt').write_text(f'{digest}  {archive_name}\n')
        commands = root / 'commands'
        commands.mkdir()
        curl = commands / 'curl'
        curl.write_text('#!/usr/bin/env python3\n'
                        'import os, pathlib, shutil, sys\n'
                        'args = sys.argv[1:]\n'
                        'output = args[args.index("--output") + 1]\n'
                        'source = pathlib.Path(os.environ["SC_LINT_TEST_ASSETS"]) / args[-1].rsplit("/", 1)[-1]\n'
                        'shutil.copyfile(source, output)\n')
        curl.chmod(0o755)
        # The action has one literal shell block per named platform step;
        # extract that block without adding a YAML dependency to Clippy CI.
        block = ACTION.read_text().split('    - name: Install sc-lint release (Unix)\n', 1)[1]
        block = block.split('      run: |\n', 1)[1].split('\n    - name:', 1)[0]
        script = root / 'install.sh'
        script.write_text(textwrap.dedent(block))
        runner_temp = root / 'runner'
        runner_temp.mkdir()
        path_file = root / 'github-path'
        environment = dict(os.environ, RUNNER_OS='Linux', RUNNER_ARCH='X64',
                           RUNNER_TEMP=str(runner_temp), SC_LINT_VERSION='0.8.9',
                           SC_LINT_REPOSITORY='randlee/sc-lint', GITHUB_PATH=str(path_file),
                           SC_LINT_TEST_ASSETS=str(assets), PATH=str(commands) + os.pathsep + os.environ['PATH'])
        process = subprocess.run(['bash', str(script)], cwd=ROOT, env=environment,
                                 capture_output=True, text=True, timeout=30)
        expected = runner_temp / 'sc-lint-0.8.9/x86_64-unknown-linux-gnu' / prefix / 'bin'
        return process, path_file, expected

    def test_nested_archive_adds_the_directory_containing_all_four_binaries(self):
        with tempfile.TemporaryDirectory() as directory:
            result, path_file, expected = self.run_install(directory)
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            self.assertEqual(path_file.read_text().splitlines(), [str(expected)])
            for binary in BINARIES:
                self.assertTrue(os.access(expected / binary, os.X_OK), binary)

    def test_checksum_mismatch_stops_before_extraction_or_path_update(self):
        with tempfile.TemporaryDirectory() as directory:
            result, path_file, expected = self.run_install(directory, corrupt_checksum=True)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn('SHA-256 mismatch', result.stderr)
            self.assertFalse(expected.exists())
            self.assertFalse(path_file.exists())

    def test_each_missing_binary_is_rejected_before_path_update(self):
        for binary in BINARIES:
            with self.subTest(binary=binary), tempfile.TemporaryDirectory() as directory:
                result, path_file, _ = self.run_install(directory, missing=binary)
                self.assertNotEqual(result.returncode, 0)
                self.assertIn(f'missing executable {binary}' if binary == 'sc-lint' else f'missing sibling backend {binary}', result.stderr)
                self.assertFalse(path_file.exists())

    def test_published_version_field_is_checked_and_mismatches_fail(self):
        block = ACTION.read_text().split('    - name: Verify sc-lint version contract\n', 1)[1]
        block = block.split('      run: |\n', 1)[1].split('\n    - name:', 1)[0]
        cases = (
            ({'ok': True, 'data': {'version': '0.8.9'}}, True),
            ({'ok': True, 'data': {'version': '0.8.9', 'self_contained': True}}, True),
            ({'ok': True, 'data': {'version': '0.8.9', 'self_contained': 'true'}}, False),
            ({'ok': True, 'data': {'version': '0.8.8'}}, False),
            ({'ok': False, 'data': {'version': '0.8.9'}}, False),
            ({'ok': True, 'data': {'crate_version': '0.8.9'}}, False),
        )
        for payload, accepted in cases:
            with self.subTest(payload=payload), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                command = root / 'sc-lint'
                command.write_text('#!/usr/bin/env python3\nimport json\nprint(' + repr(json.dumps(payload)) + ')\n')
                command.chmod(0o755)
                env = dict(os.environ, SC_LINT_VERSION='0.8.9', GITHUB_ENV=str(root / 'github-env'), PATH=str(root) + os.pathsep + os.environ['PATH'])
                result = subprocess.run(['bash', '-c', textwrap.dedent(block)], cwd=ROOT, env=env,
                                        capture_output=True, text=True, timeout=30)
                self.assertEqual(result.returncode == 0, accepted, result.stdout + result.stderr)
                if not accepted:
                    self.assertIn('version contract failed', result.stderr)
                else:
                    expected = 'true' if payload['data'].get('self_contained') is True else 'false'
                    self.assertEqual((root / 'github-env').read_text().strip(), 'SC_LINT_SELF_CONTAINED=' + expected)

    def test_root_probe_selects_family_and_distinguishes_findings_from_errors(self):
        block = ACTION.read_text().split('    - name: Smoke-test repository root discovery (Unix)\n', 1)[1]
        block = block.split('      run: |\n', 1)[1].split('\n    - name:', 1)[0]
        cases = (
            (0, {'ok': True, 'data': {'status': 'pass'}}, True),
            (10, {'ok': True, 'data': {'status': 'fail', 'findings': ['controlled finding']}}, True),
            (3, {'ok': False, 'error': {'code': 'config.boundaries_dir_missing'}}, False),
            (0, {'ok': False, 'error': {'code': 'CLI.CONFIG_ERROR'}}, False),
            (7, {'ok': True, 'data': {'status': 'pass'}}, False),
        )
        for code, payload, accepted in cases:
            with self.subTest(code=code, payload=payload), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                argv = root / 'argv.json'
                command = root / 'sc-lint'
                command.write_text('#!/usr/bin/env python3\nimport json,sys\n'
                                   'open(' + repr(str(argv)) + ', "w").write(json.dumps(sys.argv[1:]))\n'
                                   'print(' + repr(json.dumps(payload)) + ')\nsys.exit(' + str(code) + ')\n')
                command.chmod(0o755)
                env = dict(os.environ, GITHUB_WORKSPACE=str(root), SC_LINT_SMOKE_FAMILY='sc-runtime',
                           PATH=str(root) + os.pathsep + os.environ['PATH'])
                result = subprocess.run(['bash', '-c', textwrap.dedent(block)], cwd=ROOT, env=env,
                                        capture_output=True, text=True, timeout=30)
                self.assertEqual(result.returncode == 0, accepted, result.stdout + result.stderr)
                self.assertEqual(json.loads(argv.read_text()), ['--json', '--root', str(root), 'lint', 'sc-runtime'])
                if code == 10:
                    self.assertIn('controlled finding', result.stdout)

    @unittest.skipUnless(shutil.which('pwsh'), 'PowerShell is not installed')
    def test_windows_version_capability_is_typed_and_retains_legacy_fallback(self):
        block = ACTION.read_text().split('    - name: Verify sc-lint version contract (Windows)\n', 1)[1]
        block = block.split('      run: |\n', 1)[1].split('\n    - name:', 1)[0]
        cases = (
            ({'ok': True, 'data': {'version': '0.8.9', 'self_contained': True}}, True, 'true'),
            ({'ok': True, 'data': {'version': '0.8.9'}}, True, 'false'),
            ({'ok': True, 'data': {'version': '0.8.9', 'self_contained': 'true'}}, False, None),
            ({'ok': True, 'data': {'version': '0.8.8', 'self_contained': True}}, False, None),
        )
        for payload, accepted, flag in cases:
            with self.subTest(payload=payload), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                command = root / 'sc-lint'
                command.write_text('#!/usr/bin/env python3\nprint(' + repr(json.dumps(payload)) + ')\n')
                command.chmod(0o755)
                env = dict(os.environ, SC_LINT_VERSION='0.8.9', GITHUB_ENV=str(root / 'github-env'),
                           PATH=str(root) + os.pathsep + os.environ['PATH'])
                result = subprocess.run(['pwsh', '-NoProfile', '-NonInteractive', '-Command', textwrap.dedent(block)],
                                        cwd=ROOT, env=env, capture_output=True, text=True, timeout=30)
                self.assertEqual(result.returncode == 0, accepted, result.stdout + result.stderr)
                if accepted:
                    self.assertEqual((root / 'github-env').read_text().strip(), 'SC_LINT_SELF_CONTAINED=' + flag)


if __name__ == '__main__':
    unittest.main()
