"""Pure observer checks using sealed real frames; these do not prove TC recovery."""
import hashlib,json,tempfile,time,unittest
from pathlib import Path
from rm_proxy import expected_peer_reset
from tcc_joint_proxy import JointCrashRule

FIXTURE=json.loads(Path(__file__).with_name('tcc_dynamic_proxy_frames.json').read_bytes())
FRAMES={int(k):bytes.fromhex(v) for k,v in FIXTURE['frames'].items()}

class JointProxyTests(unittest.TestCase):
    def test_only_original_callback_is_held_and_no_response_is_synthesized(self):
        with tempfile.TemporaryDirectory(dir='D:/Temp') as tmp:
            path=Path(tmp)/'cut.json';rule=JointCrashRule(path)
            rule.inspect('c2s',1,FRAMES[103]);rule.inspect('s2c',1,FRAMES[104])
            for direction,kind in [('c2s',13),('s2c',14)]:
                _,sends,closed=rule.inspect(direction,1,FRAMES[kind]);self.assertEqual(sends,[FRAMES[kind]]);self.assertFalse(closed)
            event,sends,closed=rule.inspect('s2c',1,FRAMES[5]);self.assertEqual(sends,[]);self.assertFalse(closed);self.assertEqual((event['action'],event['delivery_count']),('hold_for_joint_crash',0))
            self.assertEqual(json.loads(path.read_bytes())['frame_hex'],FRAMES[5].hex());self.assertTrue(rule.complete(1))
            with self.assertRaises(ValueError):rule.inspect('c2s',1,FRAMES[6])
    def test_changed_callback_cannot_replace_original_held_identity(self):
        with tempfile.TemporaryDirectory(dir='D:/Temp') as tmp:
            rule=JointCrashRule(Path(tmp)/'cut.json');rule.inspect('c2s',1,FRAMES[103]);rule.inspect('s2c',1,FRAMES[5])
            changed=FRAMES[5].replace(b'"id":1',b'"id":2')
            with self.assertRaises(ValueError):rule.inspect('s2c',1,changed)
    def test_reset_requires_explicit_fresh_owned_kill_authorization(self):
        with tempfile.TemporaryDirectory(dir='D:/Temp') as tmp:
            path=Path(tmp)/'kill.json';error=ConnectionResetError(10054,'owned reset')
            self.assertIsNone(expected_peer_reset(error,None));self.assertIsNone(expected_peer_reset(error,path))
            data=dict(phase='joint-kill',writer_pid=123,tc_pid=456,monotonic_ns=time.perf_counter_ns());path.write_text(json.dumps(data),encoding='utf-8')
            self.assertEqual(expected_peer_reset(error,path),hashlib.sha256(path.read_bytes()).hexdigest())
            self.assertIsNone(expected_peer_reset(EOFError('truncated frame'),path));self.assertIsNone(expected_peer_reset(TimeoutError('deadline'),path))
            for key,value in [('phase','anything'),('writer_pid',True),('tc_pid',123),('monotonic_ns',time.perf_counter_ns()-11_000_000_000)]:
                changed=data|{key:value};path.write_text(json.dumps(changed),encoding='utf-8');self.assertIsNone(expected_peer_reset(error,path))

if __name__=='__main__':unittest.main()
