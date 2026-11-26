"""在 D 盘实际证据副本上检验封存来源门禁；不修改原运行或 SDK。"""
import json
import os
from pathlib import Path
import shutil
import unittest
import uuid

from validate_capture import validate


class CaptureTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        original = Path(os.environ['SEATA_STATUS_RUN'])
        cls.base = Path('D:/codex-builds') / ('status-guards-' + uuid.uuid4().hex[:12])
        cls.out = cls.base / 'capture'
        shutil.copytree(original, cls.out)
        runtime = cls.base / 'runtime'
        (runtime / 'classes').mkdir(parents=True)
        shutil.copyfile(cls.out / 'artifacts/GlobalStatusContractProbe.class', runtime / 'classes/GlobalStatusContractProbe.class')
        shutil.copyfile(cls.out / 'compile.args', runtime / 'compile.args')
        execution = cls.out / 'execution.json'
        metadata_path = execution if execution.exists() else cls.out / 'result.json'
        result = json.loads(metadata_path.read_bytes())
        result['runtime'] = str(runtime)
        metadata_path.write_text(json.dumps(result), encoding='utf-8')
        metadata = cls.out / 'java-probe-binary.json'
        if metadata.exists():
            value = json.loads(metadata.read_bytes())
            value['path'] = str(runtime / 'classes/GlobalStatusContractProbe.class')
            metadata.write_text(json.dumps(value), encoding='utf-8')
        cls.runtime = runtime

    def damaged(self, changes):
        saved = {path: path.read_bytes() for path in changes}
        try:
            for path, data in changes.items(): path.write_bytes(data)
            with self.assertRaises(ValueError): validate(self.out)
        finally:
            for path, data in saved.items(): path.write_bytes(data)

    def test_actual_capture(self):
        self.assertEqual(16, validate(self.out)['critical_classes'])

    def test_mismatched_raw_origin(self):
        path = self.out / 'java.json'
        value = json.loads(path.read_bytes())
        value['origins'][0]['location'] = 'file:/D:/wrong/classes/'
        self.damaged({path: json.dumps(value).encode()})

    def test_missing_class_audit(self):
        path = self.out / 'critical-class-audit.json'
        value = json.loads(path.read_bytes())[:-1]
        self.damaged({path: json.dumps(value).encode()})

    def test_corrupt_go_artifact(self):
        path = self.out / 'artifacts/go-native.exe'
        self.damaged({path: path.read_bytes() + b'changed'})

    def test_corrupt_frozen_input(self):
        row = json.loads((self.out / 'input-audit.json').read_bytes())[0]
        path = self.out / row['snapshot']
        self.damaged({path: path.read_bytes() + b'changed'})

    def test_both_probe_copies_changed(self):
        paths = [self.out / 'artifacts/GlobalStatusContractProbe.class',
                 self.runtime / 'classes/GlobalStatusContractProbe.class']
        self.damaged({path: path.read_bytes() + b'changed' for path in paths})


if __name__ == '__main__': unittest.main()
