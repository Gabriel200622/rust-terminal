import importlib.util
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

import release


class ReleaseTests(unittest.TestCase):
    def test_semver_classification_and_names(self):
        for version, pre in [('0.1.0', False), ('0.2.0-beta.1', True), ('0.2.0-rc.1+build.5', True), ('0.2.0+build.5', False)]:
            self.assertEqual(release.version_info(version)[1], pre)
            self.assertEqual(set(release.artifact_names(version).values()), {f'Neptune-{version}-{suffix}' for suffix in ['macos-arm64.dmg', 'macos-x64.dmg', 'windows-x64.exe', 'linux-x64.AppImage', 'linux-x64.deb']})
        for version in ['v0.1.0', '01.2.3', '0.1', '0.1.0-', '0.1.0-beta.01', '0.1.0+','0.1.0-a..b', '0.1.0\n', '65536.0.0']:
            with self.subTest(version=version), self.assertRaises(ValueError):
                release.version_info(version)

    def root(self, directory):
        root = Path(directory)
        (root / 'packaging').mkdir()
        (root / 'packaging/update-public-key.hex').write_text('11' * 32)
        (root / 'Cargo.toml').write_text('[package]\nname = "neptune-terminal"\nversion = "0.1.0"\n')
        (root / 'Cargo.lock').write_text('version = 4\n[[package]]\nname = "neptune-terminal"\nversion = "0.1.0"\n')
        (root / 'CHANGELOG.md').write_text('# Changelog\n\n## [Unreleased]\n\n### What\'s New\n\n- Better terminal startup.\n')
        return root

    def test_prepare_validate_and_single_notes_source(self):
        with tempfile.TemporaryDirectory() as directory:
            root = self.root(directory)
            release.prepare('0.2.0-beta.1', root)
            self.assertEqual(release.validate('v0.2.0-beta.1', root), {'version': '0.2.0-beta.1', 'prerelease': 'true'})
            self.assertEqual(release.notes('0.2.0-beta.1', root), "### What's New\n\n- Better terminal startup.\n")
            self.assertIn('## [Unreleased]', (root / 'CHANGELOG.md').read_text())
            with self.assertRaises(ValueError): release.validate('v0.2.0', root)
            with self.assertRaises(ValueError): release.prepare('0.2.0-beta.1', root)
            (root / 'Cargo.lock').write_text((root / 'Cargo.lock').read_text().replace('0.2.0-beta.1', '0.1.0'))
            with self.assertRaises(ValueError): release.validate('v0.2.0-beta.1', root)

    def test_incomplete_release_cannot_generate_manifest(self):
        with tempfile.TemporaryDirectory() as directory:
            with self.assertRaisesRegex(ValueError, 'incomplete'):
                release.manifest('0.2.0', Path(directory), 'a' * 40)

    def test_signed_manifest_and_checksums_match_exact_final_bytes(self):
        with tempfile.TemporaryDirectory() as directory:
            root = self.root(directory)
            release.prepare('0.2.0', root)
            dist = root / 'dist'
            dist.mkdir()
            for name in release.artifact_names('0.2.0').values(): (dist / name).write_bytes(b'final installer bytes')
            key = root / 'private.pem'
            subprocess.run(['openssl', 'genpkey', '-algorithm', 'ED25519', '-out', str(key)], check=True)
            public = subprocess.check_output(['openssl', 'pkey', '-in', str(key), '-pubout', '-outform', 'DER'])
            (root / 'packaging/update-public-key.hex').write_text(public[-32:].hex())
            with patch.object(release, 'ROOT', root), patch.object(release, 'notes', return_value=release.notes('0.2.0', root)), patch.dict(os.environ, {'UPDATE_SIGNING_KEY': key.read_text()}):
                release.manifest('0.2.0', dist, 'a' * 40)
            manifest = json.loads((dist / 'update-manifest.json').read_text())
            self.assertEqual(manifest['version'], '0.2.0')
            for asset in manifest['assets']:
                self.assertEqual(asset['sha256'], release.sha256(dist / asset['name']))
                self.assertEqual(asset['size'], len(b'final installer bytes'))
            for line in (dist / 'SHA256SUMS').read_text().splitlines():
                digest, name = line.split('  ')
                self.assertEqual(digest, release.sha256(dist / name))
            signature = root / 'raw.sig'
            signature.write_bytes(bytes.fromhex((dist / 'update-manifest.sig').read_text()))
            public_pem = root / 'public.pem'
            subprocess.run(['openssl', 'pkey', '-in', str(key), '-pubout', '-out', str(public_pem)], check=True)
            subprocess.run(['openssl', 'pkeyutl', '-verify', '-pubin', '-inkey', str(public_pem), '-rawin', '-in', str(dist / 'update-manifest.json'), '-sigfile', str(signature)], check=True, stdout=subprocess.DEVNULL)

    def test_published_release_rerun_never_mutates_public_assets(self):
        with patch.object(release, 'validate', return_value={'version': '0.2.0', 'prerelease': 'false'}), patch.object(release.subprocess, 'run', return_value=subprocess.CompletedProcess([], 0, '{"draft":false}', '')) as run:
            with self.assertRaisesRegex(ValueError, 'already published'):
                release.stage('v0.2.0', Path('dist'))
            self.assertEqual(run.call_count, 1)

    def test_release_lookup_errors_fail_closed(self):
        with patch.object(release, 'validate', return_value={'version': '0.2.0', 'prerelease': 'false'}), patch.object(release.subprocess, 'run', return_value=subprocess.CompletedProcess([], 1, '', 'HTTP 403')) as run:
            with self.assertRaisesRegex(ValueError, 'safely determine'):
                release.stage('v0.2.0', Path('dist'))
            self.assertEqual(run.call_count, 1)

    def test_upload_failure_remains_a_private_draft(self):
        with tempfile.TemporaryDirectory() as directory:
            dist = Path(directory)
            (dist / 'asset').write_bytes(b'installer')
            calls = []
            def run(command, **kwargs):
                calls.append(command)
                if command[:2] == ['gh', 'api']:
                    return subprocess.CompletedProcess(command, 1, '', 'HTTP 404')
                if command[:3] == ['gh', 'release', 'upload']:
                    raise subprocess.CalledProcessError(1, command)
                return subprocess.CompletedProcess(command, 0)
            with patch.object(release, 'validate', return_value={'version': '0.2.0', 'prerelease': 'false'}), patch.object(release.subprocess, 'run', side_effect=run):
                with self.assertRaises(subprocess.CalledProcessError): release.stage('v0.2.0', dist)
            self.assertTrue(all('--draft' in c for c in calls if c[:3] in [['gh', 'release', 'create'], ['gh', 'release', 'edit']]))
            self.assertFalse(any('--draft=false' in c for c in calls))

    def test_completed_draft_gets_changelog_notes_only_after_all_uploads(self):
        with tempfile.TemporaryDirectory() as directory:
            root = self.root(directory)
            release.prepare('0.2.0-beta.1', root)
            notes = root / 'release-notes.md'
            notes.write_text(release.notes('0.2.0-beta.1', root))
            dist = root / 'dist'
            dist.mkdir()
            names = [*release.artifact_names('0.2.0-beta.1').values(), 'SHA256SUMS', 'update-manifest.json', 'update-manifest.sig']
            for name in names: (dist / name).write_bytes(b'fixture')
            calls = []
            def run(command, **kwargs):
                calls.append(command)
                if command[:2] == ['gh', 'api']:
                    return subprocess.CompletedProcess(command, 1, '', 'HTTP 404')
                return subprocess.CompletedProcess(command, 0)
            complete = json.dumps({'draft': True, 'assets': [{'name': name} for name in names]}).encode()
            with patch.object(release, 'validate', return_value=release.validate('v0.2.0-beta.1', root)), patch.object(release.subprocess, 'run', side_effect=run), patch.object(release.subprocess, 'check_output', return_value=complete):
                release.stage('v0.2.0-beta.1', dist)
            upload_index = next(i for i, command in enumerate(calls) if command[:3] == ['gh', 'release', 'upload'])
            self.assertTrue(all('INCOMPLETE' in ' '.join(command) for command in calls[1:upload_index]))
            final = calls[-1]
            self.assertIn('--draft', final)
            self.assertIn('--prerelease=true', final)
            self.assertEqual(final[-2:], ['--notes-file', str(notes)])
            self.assertFalse(any('--draft=false' in command for command in calls))

    def test_macos_missing_credentials_cannot_fall_back_to_unsigned_artifacts(self):
        spec = importlib.util.spec_from_file_location('sign_macos', Path(__file__).with_name('sign-macos.py'))
        signing = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(signing)
        with patch.dict(os.environ, {}, clear=True), patch('sys.argv', ['sign-macos.py', '--version', '0.2.0', '--platform', 'macos-arm64', '--binary', 'fixture']), patch.object(signing, 'run') as run:
            with self.assertRaisesRegex(ValueError, 'Missing macOS release credentials'):
                signing.main()
            run.assert_not_called()


if __name__ == '__main__': unittest.main()
