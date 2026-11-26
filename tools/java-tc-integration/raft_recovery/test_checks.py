import copy
import unittest
import checks


def state():
    return dict(leader=True,node_id='default/192.168.1.10:28092',term=2,committed=30,applied=30,all_sessions=2,
        records=[dict(xid='192.168.1.10:28091:940000',exists=True,resource='mysql://raft-retry-0',lockable=False,status=16,branches=1,branch_id=950000,branch_type=0,branch_resource='mysql://raft-retry-0',branch_lock_key='orders:1',branch_xid='192.168.1.10:28091:940000'),
                 dict(xid='192.168.1.10:28091:940001',exists=True,resource='mysql://raft-retry-1',lockable=True,status=16,branches=0,branch_id=0,branch_type=-1,branch_resource=None,branch_lock_key=None,branch_xid=None)])


class EvidenceGateTest(unittest.TestCase):
    def test_original_identity_status_branch_and_lock_must_all_match(self):
        self.assertTrue(checks.verify_records(state(),16,'192.168.1.10'))
        for field,value in [('xid','other'),('status',15),('branch_id',999),('branches',0),('lockable',True),('resource','other'),('exists',False),('branch_type',1),('branch_resource','other'),('branch_lock_key','orders:9'),('branch_xid','other')]:
            changed=state();changed['records'][0][field]=value
            with self.subTest(field=field),self.assertRaises(RuntimeError):checks.verify_records(changed,16,'192.168.1.10')
        changed=state();changed['records'].pop()
        with self.assertRaises(RuntimeError):checks.verify_records(changed,16,'192.168.1.10')
        changed=state();del changed['all_sessions']
        with self.assertRaises(RuntimeError):checks.verify_records(changed,16,'192.168.1.10')

    def test_leader_transition_requires_another_node_later_term_and_replicated_index(self):
        before=state();after=copy.deepcopy(before);after.update(node_id='default/192.168.1.10:28094',term=3,committed=31,applied=31)
        self.assertTrue(checks.verify_transition(before,after,before['node_id']))
        for field,value in [('node_id',before['node_id']),('term',2),('committed',29),('applied',29),('leader',False)]:
            changed=copy.deepcopy(after);changed[field]=value
            with self.subTest(field=field),self.assertRaises(RuntimeError):checks.verify_transition(before,changed,before['node_id'])

    def test_only_exact_exception_status_and_original_async_reload_stack_are_evidence(self):
        event=dict(type='jdk.JavaExceptionThrow',values=dict(startTime='2026-11-12T08:00:01Z',thrownClass={'name':'org/apache/seata/common/exception/ShouldNeverHappenException'},message='NOT properly handled CommitRetryTimeout',
            stackTrace={'frames':[{'method':{'type':{'name':'org/apache/seata/server/session/SessionHolder'},'name':'reload'}},
                                  {'method':{'type':{'name':'org/apache/seata/server/cluster/raft/RaftStateMachine'},'name':'lambda$onLeaderStart$0'}}]}))
        self.assertEqual(checks.reload_exceptions([event],16),[event])
        class_loading=copy.deepcopy(event)
        class_loading['values'].update(thrownClass={'name':'java/lang/ClassNotFoundException'},message='org.apache.seata.common.exception.ShouldNeverHappenException')
        self.assertEqual(checks.reload_exceptions([class_loading],1,'2026-11-12T08:00:00Z'),[])
        self.assertEqual(checks.reload_exceptions([event],1,'2026-11-12T08:00:00Z'),[event])
        self.assertEqual(checks.reload_exceptions([event],17),[])
        for path,value in [('message','backend failed'),('thrownClass',{'name':'java/lang/IllegalArgumentException'}),('stackTrace',{'frames':[]})]:
            changed=copy.deepcopy(event);changed['values'][path]=value
            self.assertEqual(checks.reload_exceptions([changed],16),[])
        self.assertEqual(checks.reload_exceptions([event],16,'2026-11-12T08:00:02Z'),[])
        self.assertEqual(checks.reload_exceptions([event],16,'2026-11-12T08:00:00Z'),[event])
        calls=[]
        def read(index):
            calls.append(index)
            return [] if index==0 else [event]
        found,index=checks.wait_for_reload(read,16,'2026-11-12T08:00:00Z',max_attempts=3,delay=0)
        self.assertEqual(found,[event]);self.assertEqual(index,1);self.assertEqual(calls,[0,1])
        with self.assertRaises(RuntimeError):checks.wait_for_reload(lambda _: [event],16,'2026-11-12T08:00:02Z',max_attempts=2,delay=0)


if __name__=='__main__':unittest.main()
