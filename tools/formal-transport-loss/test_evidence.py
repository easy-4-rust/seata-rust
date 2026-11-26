"""破坏既有实跑的原始帧，验证观察器不会仅凭元数据/PASS行误通过。"""
import json
from pathlib import Path
import struct
import unittest
from evidence_audit import validate


ROOT = Path(__file__).resolve().parents[2]
BASE = ROOT / 'docs/phase-1/evidence/transport-loss-java/run_20261113_211233_67c81a72'


class RawEvidenceControls(unittest.TestCase):
    def evidence(self, mode='commit'):
        case = BASE / mode
        rows = [json.loads(line) for line in (case / 'events.jsonl').read_text(encoding='utf-8').splitlines()]
        text = (case / 'client.log').read_text(encoding='utf-8')
        nonce = json.loads((case / 'owned-processes.json').read_bytes())['nonce']
        return rows, text, nonce

    def test_actual_three_case_frames_remain_accepted(self):
        for mode in ['begin', 'commit', 'branch-register']:
            rows, text, nonce = self.evidence(mode)
            validate(mode, text, rows, nonce)

    def test_raw_callback_reply_identity_cannot_hide_behind_unchanged_metadata(self):
        for field in ['xid', 'branch_id', 'wire_id', 'branch_status', 'connection']:
            with self.subTest(field=field):
                rows, text, nonce = self.evidence()
                reply = next(row for row in rows if row.get('kind') == 4)
                raw = bytearray.fromhex(reply['frame_hex'])
                head = struct.unpack('>H', raw[7:9])[0]
                xid_length = struct.unpack('>H', raw[head+4:head+6])[0]
                if field == 'xid': raw[head+6] ^= 1
                elif field == 'branch_id': raw[head+6+xid_length+7] ^= 1
                elif field == 'wire_id': raw[15] ^= 1
                elif field == 'branch_status': raw[-1] = 6
                else: reply['connection'] += 100
                reply['frame_hex'] = raw.hex()
                with self.assertRaises(ValueError): validate('commit', text, rows, nonce)

    def test_commit_callback_requires_original_successful_registration_branch(self):
        rows, text, nonce = self.evidence()
        reply = next(row for row in rows if row.get('kind') == 12)
        raw = bytearray.fromhex(reply['frame_hex']); raw[-1] ^= 1
        reply['frame_hex'] = raw.hex()
        reply['branch_id'] ^= 1
        with self.assertRaises(ValueError): validate('commit', text, rows, nonce)

    def test_lock_results_require_same_registered_resource_and_lock_key(self):
        for field in ['resource', 'lock', 'response_wire_id']:
            with self.subTest(field=field):
                rows, text, nonce = self.evidence()
                kind = 22 if field == 'response_wire_id' else 21
                row = next(row for row in rows if row.get('kind') == kind)
                raw = bytearray.fromhex(row['frame_hex'])
                if field == 'response_wire_id': raw[15] ^= 1
                else:
                    needle = b'mysql://auth-a' if field == 'resource' else b'orders:1'
                    at = raw.index(needle); raw[at+len(needle)-1] ^= 1
                row['frame_hex'] = raw.hex()
                with self.assertRaises(ValueError): validate('commit', text, rows, nonce)

    def test_commit_status_claims_are_bound_to_actual_delivered_responses(self):
        for field in ['dropped_status', 'observed_status', 'retry_status']:
            with self.subTest(field=field):
                rows, text, nonce = self.evidence()
                if field == 'dropped_status': row = next(row for row in rows if row.get('dropped'))
                elif field == 'retry_status': row = next(row for row in rows if row.get('kind') == 8 and not row['dropped'])
                else: row = [row for row in rows if row.get('kind') == 16][-1]
                raw = bytearray.fromhex(row['frame_hex']); raw[-1] = 9 if field != 'dropped_status' else 15
                row['frame_hex'] = raw.hex(); row['global_status'] = raw[-1]
                with self.assertRaises(ValueError): validate('commit', text, rows, nonce)

    def test_pending_branch_cancellation_requires_actual_rollback_acknowledgement(self):
        rows, text, nonce = self.evidence('branch-register')
        row = next(row for row in rows if row.get('kind') == 10)
        raw = bytearray.fromhex(row['frame_hex']); raw[-1] = 9
        row['frame_hex'] = raw.hex(); row['global_status'] = 9
        with self.assertRaises(ValueError): validate('branch-register', text, rows, nonce)


if __name__ == '__main__': unittest.main()
