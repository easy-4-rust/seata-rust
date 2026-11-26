"""用实际 JVM/Go 运行输出校验证据门禁。"""
import copy
import json
import os
from pathlib import Path
import unittest

import verify


class EvidenceTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        root = Path(os.environ['SEATA_STATUS_RUN'])
        cls.java = json.loads((root / 'java.json').read_bytes())
        cls.native = json.loads((root / 'go-native.json').read_bytes())
        cls.profile = json.loads((root / 'go-profile.json').read_bytes())

    def check(self, java=None, native=None, profile=None):
        return verify.check(java or self.java, native or self.native, profile or self.profile)

    def test_actual_run(self):
        self.assertEqual(168, self.check()['response_cases'])

    def test_missing_status(self):
        value = copy.deepcopy(self.java)
        value['statuses'].pop()
        with self.assertRaises(ValueError): self.check(java=value)

    def test_wrong_classification(self):
        value = copy.deepcopy(self.java)
        value['statuses'][18]['two_phase_success'] = False
        with self.assertRaises(ValueError): self.check(java=value)

    def test_duplicate_response(self):
        value = copy.deepcopy(self.java)
        value['responses'][-1] = value['responses'][0]
        with self.assertRaises(ValueError): self.check(java=value)

    def test_invalid_transition_claim(self):
        value = copy.deepcopy(self.java)
        value['transitions'][2 * 21 + 4]['allowed'] = True
        with self.assertRaises(ValueError): self.check(java=value)

    def test_failed_result_overwritten(self):
        value = copy.deepcopy(self.native)
        row = next(r for r in value['responses'] if r['result'] == 0)
        row['result'] = 1
        with self.assertRaises(ValueError): self.check(native=value)

    def test_wire_changed(self):
        value = copy.deepcopy(self.profile)
        value['responses'][0]['encoded_hex'] += '00'
        with self.assertRaises(ValueError): self.check(profile=value)

    def test_missing_unknown(self):
        value = copy.deepcopy(self.profile)
        value['unknown'].pop()
        with self.assertRaises(ValueError): self.check(profile=value)

    def test_unknown_accepted(self):
        value = copy.deepcopy(self.profile)
        value['unknown'][0]['rejected'] = False
        with self.assertRaises(ValueError): self.check(profile=value)

    def test_invented_go_name(self):
        value = copy.deepcopy(self.native)
        value['named_statuses'].append(16)
        with self.assertRaises(ValueError): self.check(native=value)

    def test_wrong_java_invalid_status(self):
        value = copy.deepcopy(self.java)
        value['unknown'][0]['int_rejected'] = False
        with self.assertRaises(ValueError): self.check(java=value)


if __name__ == '__main__': unittest.main()
