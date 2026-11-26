"""Portable full-baseline boundaries must reject escaping or replaced inputs."""
import hashlib
import json
from pathlib import Path
import shutil
import unittest
import uuid
import zipfile

from inventory_rebuilt import safe_path
from portable_rebuilt import classify_input, validate_members, verify_package_authority
from replay_rebuilt import validate_origins


class RebuiltInputsTest(unittest.TestCase):
    def test_canonical_members_stay_under_new_workspace(self):
        root = Path('D:/codex-builds/portable-test')
        self.assertEqual(safe_path(root, 'm2/org/example/1.0/example.jar'),
                         root / 'm2/org/example/1.0/example.jar')

    def test_escaping_and_noncanonical_members_are_rejected(self):
        for name in ('', '../x', '/x', 'a//x', 'a/./x', 'a/../x', 'E:/x',
                     'F:/x', 'a\\x', 'a/C:/x'):
            with self.subTest(name=name), self.assertRaises(ValueError):
                safe_path(Path('D:/codex-builds/portable-test'), name)

    def test_only_original_stage_and_repository_are_packaged(self):
        stage = Path('D:/codex-builds/stage')
        repo = Path('D:/codex-builds/m2')
        self.assertEqual(classify_input(stage / 'source/x.class', stage, repo), 'stage/source/x.class')
        self.assertEqual(classify_input(repo / 'org/x.jar', stage, repo), 'm2/org/x.jar')
        for path in (Path('C:/unowned/x'), Path('F:/x'), stage.parent / 'stage-other/x'):
            with self.subTest(path=path), self.assertRaises(ValueError):
                classify_input(path, stage, repo)

    def test_self_rehashed_or_empty_manifest_is_not_authority(self):
        for data in (b'{}', json.dumps({'files': [], 'sha256': 'self-rehashed'}).encode()):
            with self.assertRaises(ValueError):
                verify_package_authority(data)

    def test_member_structure_and_content_are_checked_before_extraction(self):
        root = Path('D:/codex-builds') / ('package-member-test-' + uuid.uuid4().hex)
        root.mkdir()
        try:
            data = b'pinned member'
            for label, member, mode, digest, size, total, duplicate in [
                ('valid', 'stage/source/value', 0o100644, hashlib.sha256(data).hexdigest(), len(data), len(data), False),
                ('escape', '../outside', 0o100644, hashlib.sha256(data).hexdigest(), len(data), len(data), False),
                ('symlink', 'stage/source/value', 0o120777, hashlib.sha256(data).hexdigest(), len(data), len(data), False),
                ('wrong-bytes', 'stage/source/value', 0o100644, '0' * 64, len(data), len(data), False),
                ('wrong-size', 'stage/source/value', 0o100644, hashlib.sha256(data).hexdigest(), len(data) + 1, len(data), False),
                ('wrong-total', 'stage/source/value', 0o100644, hashlib.sha256(data).hexdigest(), len(data), len(data) + 1, False),
                ('duplicate', 'stage/source/value', 0o100644, hashlib.sha256(data).hexdigest(), len(data), len(data), True),
            ]:
                with self.subTest(label=label):
                    with zipfile.ZipFile(root / 'runtime-inputs.zip', 'w') as archive:
                        info = zipfile.ZipInfo(member)
                        info.external_attr = mode << 16
                        archive.writestr(info, data)
                    row = dict(member=member, bytes=size, sha256=digest)
                    manifest = dict(files=[row, row] if duplicate else [row], total_bytes=total)
                    if label == 'valid':
                        validate_members(root, manifest)
                    else:
                        with self.assertRaises(ValueError):
                            validate_members(root, manifest)
        finally:
            self.assertTrue(root.resolve().is_relative_to(Path('D:/codex-builds').resolve()))
            shutil.rmtree(root)


class OriginHistoryTest(unittest.TestCase):
    def test_later_defined_copy_does_not_erase_original_file_owner(self):
        root = Path('D:/codex-builds') / ('origin-history-test-' + uuid.uuid4().hex)
        root.mkdir()
        try:
            classes = root / 'classes'
            classes.mkdir()
            jar = root / 'reflectasm-1.11.9.jar'
            jar.write_bytes(b'unit-test-generator-identity')
            engine = (root / 'junit-jupiter-engine.jar').as_uri()
            launcher = (root / 'junit-platform-launcher.jar').as_uri()
            owner = 'org.apache.seata.example.Original'
            lines = [
                '[class,load] org.junit.jupiter.engine.JupiterTestEngine source: ' + engine,
                '[class,load] org.junit.platform.launcher.core.LauncherFactory source: ' + launcher,
                '[class,load] ' + owner + ' source: ' + classes.as_uri() + '/',
                '[class,load] ' + owner + 'ConstructorAccess source: ' + jar.as_uri(),
                '[class,load] ' + owner + ' source: __JVM_DefineClass__',
            ]
            (root / 'unit-class-load.log').write_text('\n'.join(lines), encoding='utf-8')
            proof = validate_origins(root, 'unit', [classes, jar],
                [dict(jupiter_origin=engine, launcher_origin=launcher)],
                {hashlib.sha256(jar.read_bytes()).hexdigest()})
            self.assertEqual(proof['verified_generator_derivatives'][0]['source_owner'], owner)
            # Seeing the owner is insufficient if the generator is not authenticated.
            with self.assertRaises(ValueError):
                validate_origins(root, 'unit', [classes, jar],
                    [dict(jupiter_origin=engine, launcher_origin=launcher)], set())
        finally:
            self.assertTrue(root.resolve().is_relative_to(Path('D:/codex-builds').resolve()))
            shutil.rmtree(root)


if __name__ == '__main__':
    unittest.main()
