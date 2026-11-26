"""Reject candidate source drift and undeclared production files; scratch stays on D:."""
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('preface_audit_driver', HERE / 'run.py')
driver = importlib.util.module_from_spec(spec)
spec.loader.exec_module(driver)


class SourceAuditTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='preface-audit-', dir='D:/Temp')
        self.build = Path(self.temp.name)
        self.manifest = json.loads(driver.MANIFEST.read_bytes())
        self.original = []
        for row in self.manifest['files']:
            self.put(row['path'], driver.prepare.files()[row['path']])
            if row['original_lf_sha256'] is not None:
                self.original.append(dict(path=row['path'], lf_sha256=row['original_lf_sha256']))
        for path, content in [(self.manifest['existing_interface_unchanged'], b'original interface\n'),
                              ('common/src/main/java/Preserved.java', b'preserved production source\n')]:
            self.put(path, content)
            self.original.append(dict(path=path, lf_sha256=driver.digest(content)))

    def tearDown(self):
        assert self.build.resolve().is_relative_to(Path('D:/Temp').resolve())
        self.temp.cleanup()

    def put(self, path, content):
        target = self.build / path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(content)

    def test_exact_candidate_and_original_interface_are_accepted(self):
        self.assertEqual(len(driver.audit_candidate(self.build, self.original, self.manifest)), 7)

    def test_unrelated_source_and_public_interface_drift_are_rejected(self):
        for path in ['common/src/main/java/Preserved.java', self.manifest['existing_interface_unchanged']]:
            target = self.build / path
            original = target.read_bytes()
            self.put(path, b'unapproved edit\n')
            with self.subTest(path=path), self.assertRaises(RuntimeError):
                driver.audit_candidate(self.build, self.original, self.manifest)
            target.write_bytes(original)

    def test_undeclared_main_file_and_pom_are_rejected(self):
        for path in ['core/src/main/java/Unexpected.java', 'unexpected/pom.xml']:
            self.put(path, b'unapproved file\n')
            with self.subTest(path=path), self.assertRaises(RuntimeError):
                driver.audit_candidate(self.build, self.original, self.manifest)
            (self.build / path).unlink()

    def test_new_interface_missing_or_changed_is_rejected(self):
        relative = next(row['path'] for row in self.manifest['files'] if row['original_lf_sha256'] is None)
        target = self.build / relative
        target.unlink()
        with self.assertRaises(RuntimeError):
            driver.audit_candidate(self.build, self.original, self.manifest)
        self.put(relative, b'wrong implementation\n')
        with self.assertRaises(RuntimeError):
            driver.audit_candidate(self.build, self.original, self.manifest)


if __name__ == '__main__':
    unittest.main()
