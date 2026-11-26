import unittest
import run


class RepeatOperatorRejectionTest(unittest.TestCase):
    def setUp(self):
        self.xid='127.0.0.1:28091:920001'
        self.log=(f'manual operation to change the global session, xid: {self.xid}\n'
                  'Caused by: java.lang.IllegalArgumentException: Global session is not exist, may be finished\n').encode()

    def test_exact_pinned_absent_session_signature_is_required(self):
        self.assertTrue(run.is_missing_session_rejection(500,b'',self.log,self.xid))

    def test_unrelated_errors_or_wrong_transaction_are_not_rejection_evidence(self):
        for status,raw,log in [(404,b'',self.log),(500,b'broken JSON',self.log),
                               (500,b'',b'backend unavailable'),(200,b'',self.log)]:
            self.assertFalse(run.is_missing_session_rejection(status,raw,log,self.xid))
        self.assertFalse(run.is_missing_session_rejection(500,b'',self.log,'different-xid'))

    def test_old_unrelated_missing_session_log_alone_does_not_pass(self):
        self.assertFalse(run.is_missing_session_rejection(500,b'',self.log.split(b'\n',1)[1],self.xid))


class ObserverCleanupIdentityTest(unittest.TestCase):
    def test_original_or_wrong_transaction_is_never_an_observer_cleanup_target(self):
        for text in ['BACKEND_LOCK targetXid=old contenderXid=old lockable=false expected=false\n'*2,
                     'BACKEND_LOCK targetXid=wrong contenderXid=127.0.0.1:28091:100 lockable=false expected=false\n'*2]:
            with self.assertRaises(RuntimeError):run.contenders_for_cleanup(text,'old')

    def test_only_two_distinct_recorded_observers_are_selected(self):
        text=''.join(f'BACKEND_LOCK targetXid=old contenderXid=127.0.0.1:28091:{value} lockable=true expected=true\n' for value in [100,101])
        self.assertEqual(run.contenders_for_cleanup(text,'old'),['127.0.0.1:28091:100','127.0.0.1:28091:101'])
        with self.assertRaises(RuntimeError):run.contenders_for_cleanup(text.splitlines()[0]+'\n'+text.splitlines()[0],'old')


if __name__=='__main__':unittest.main()
