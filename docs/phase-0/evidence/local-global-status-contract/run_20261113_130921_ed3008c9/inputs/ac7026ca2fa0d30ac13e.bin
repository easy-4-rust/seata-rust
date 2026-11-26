"""实际运行的 Java 类来源集合必须完整且位于允许的磁盘。"""
import copy
import json
import os
from pathlib import Path
import unittest

import verify


class OriginTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        root = Path(os.environ['SEATA_STATUS_RUN'])
        cls.values = [json.loads((root / name).read_bytes()) for name in
                      ['java.json', 'go-native.json', 'go-profile.json']]

    def check(self, origins):
        values = copy.deepcopy(self.values)
        values[0]['origins'] = origins
        return verify.check(*values)

    def test_actual_complete_origins(self):
        self.assertEqual(168, verify.check(*self.values)['response_cases'])

    def test_empty_origins(self):
        with self.assertRaises(ValueError): self.check([])

    def test_duplicate_origin(self):
        origins = copy.deepcopy(self.values[0]['origins'])
        origins[-1] = origins[0]
        with self.assertRaises(ValueError): self.check(origins)

    def test_wrong_class(self):
        origins = copy.deepcopy(self.values[0]['origins'])
        origins[-1]['class'] = 'unverified.Replacement'
        with self.assertRaises(ValueError): self.check(origins)

    def test_forbidden_disk(self):
        origins = copy.deepcopy(self.values[0]['origins'])
        origins[0]['location'] = 'file:/C:/unverified/classes/'
        with self.assertRaises(ValueError): self.check(origins)


if __name__ == '__main__': unittest.main()
