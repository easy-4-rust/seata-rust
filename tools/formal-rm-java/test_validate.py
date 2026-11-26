"""Synthetic raw records test the verifier; these fixtures are never counted as a Java execution."""
import copy
import importlib.util
from pathlib import Path
import struct
import unittest
import zlib

spec=importlib.util.spec_from_file_location('formal_rm_validator',Path(__file__).with_name('validate.py'))
validator=importlib.util.module_from_spec(spec);spec.loader.exec_module(validator)
NONCE='a'*32
XID='tc:9007199254740993'
CONTENDER='tc:9007199254740994'
BRANCH=9007199254740995
APP='{"formalRm":"'+NONCE+'"}'
def string(value,wide=False):
    raw=value.encode() if value is not None else b'';return struct.pack('>i' if wide else '>h',len(raw))+raw
def frame(kind,request_type,request_id,body):
    payload=struct.pack('>H',kind)+body
    return b'\xda\xda\x01'+struct.pack('>IHB',16+len(payload),16,request_type)+b'\x01\x00'+struct.pack('>i',request_id)+payload
def callback_payload(commit=True):
    return struct.pack('>H',3 if commit else 5)+string(XID)+struct.pack('>qB',BRANCH,0)+string('mysql://auth-a')+string(APP,True)
def journal(commit=True):
    result=b'SRMJNL01'
    for seq,state in [(1,1),(2,3)]:
        raw=struct.pack('>QB',seq,state)+callback_payload(commit);raw+=struct.pack('>I',zlib.crc32(raw));result+=struct.pack('>I',len(raw))+raw
    return result
def fixture(commit=True):
    rows=[dict(event='ready',index=0,mode='basic',pid=123,listen_port=38092,upstream_port=38091)]
    def add(direction,kind,typ,wire,body):
        raw=frame(kind,typ,wire,body);rows.append(dict(event='frame',index=len(rows),direction=direction,connection=1,kind=kind,request_type=typ,request_id=wire,action='forward',delivery_count=1,frame_hex=raw.hex()))
    add('c2s',103,0,1,string('1.1.0')+string('phase0-auth')+string('default_tx_group')+string('phase0-local-only')+string('mysql://auth-a',True));add('s2c',104,1,1,b'\x01'+string(None))
    add('c2s',11,0,2,string(XID)+b'\x00'+string('mysql://auth-a')+string('orders:1',True)+string(APP,True));add('s2c',12,1,2,b'\x01\x00'+struct.pack('>q',BRANCH))
    add('c2s',13,0,3,string(XID)+struct.pack('>qB',BRANCH,2)+string(None)+string(APP,True)+b'\x00');add('s2c',14,1,3,b'\x01\x00')
    for wire,xid,lockable in [(4,XID,True),(5,CONTENDER,False)]:
        add('c2s',21,0,wire,string(xid)+b'\x00'+string('mysql://auth-a')+string('orders:1',True)+string(APP,True));add('s2c',22,1,wire,b'\x01\x00'+struct.pack('>h',int(lockable)))
    add('s2c',3 if commit else 5,0,-7,callback_payload(commit)[2:]);add('c2s',4 if commit else 6,1,-7,b'\x01\x00'+string(XID)+struct.pack('>qB',BRANCH,5 if commit else 8))
    add('c2s',21,0,6,string(CONTENDER)+b'\x00'+string('mysql://auth-a')+string('orders:1',True)+string(APP,True));add('s2c',22,1,6,b'\x01\x00\x00\x01')
    rows.append(dict(event='summary',index=len(rows),succeeded=True,mode='basic',connections=1,faults=0,released=0,closed=0,errors=[]))
    op='commit' if commit else 'rollback'
    text=f'END mode={op} xid={XID} status={8 if commit else 11} unknown=false\nPASS RM_NATIVE mode={op} xid={XID} branch_id={BRANCH} contender={CONTENDER} nonce={NONCE} apply_calls=1 effects=1 recover_calls=0 journal_reopened=true lock_before=false lock_after=true\n'
    return text,rows,journal(commit)

def fault_fixture(mode):
    text,rows,data=fixture();rows=copy.deepcopy(rows)
    original=next(row for row in rows if row.get('kind')==3);answer=next(row for row in rows if row.get('kind')==4)
    if mode=='duplicate':
        original.update(action='duplicate_original',delivery_count=2);answer.update(action='hold',delivery_count=0)
        second=copy.deepcopy(answer);second.update(action='release_original',delivery_count=1,released_frame_hex=answer['frame_hex']);rows.insert(rows.index(answer)+1,second)
        rows[-1].update(mode='duplicate',faults=1,released=1)
    else:
        answer.update(action='drop_and_close',delivery_count=0);at=rows.index(answer)+1
        new_reg=copy.deepcopy(rows[1]);new_reg['connection']=2;new_ack=copy.deepcopy(rows[2]);new_ack['connection']=2
        new_callback=copy.deepcopy(original);new_callback['connection']=2;new_callback['request_id']=-8;raw=bytearray.fromhex(new_callback['frame_hex']);raw[12:16]=struct.pack('>i',-8);new_callback['frame_hex']=raw.hex()
        new_answer=copy.deepcopy(answer);new_answer.update(connection=2,request_id=-8,action='forward',delivery_count=1);raw=bytearray.fromhex(new_answer['frame_hex']);raw[12:16]=struct.pack('>i',-8);new_answer['frame_hex']=raw.hex()
        # 新代际回调可在104前到达，但RM必须等104后才执行/回应。
        additions=[dict(event='fault_close',connection=1),new_reg,new_callback,new_ack,new_answer];rows[at:at]=additions
        for row in rows[at+len(additions):-1]:
            if row.get('event')=='frame':row['connection']=2
        rows[-1].update(mode='reconnect',connections=2,faults=1,closed=1)
    rows[0]['mode']=mode
    for i,row in enumerate(rows):row['index']=i
    return text,rows,data

class VerifyTests(unittest.TestCase):
    def check(self,text,rows,data,case='basic-commit'):return validator.validate(case,text,rows,NONCE,data)
    def test_complete_basic_commit_and_rollback(self):
        for commit in [True,False]:self.assertEqual(self.check(*fixture(commit),case='basic-'+('commit' if commit else 'rollback'))['apply_calls'],1)
    def test_missing_or_duplicate_actual_registration_response_is_rejected(self):
        text,rows,data=fixture()
        for change in ['missing','duplicate']:
            changed=copy.deepcopy(rows)
            if change=='missing':changed.pop(4)
            else:changed.insert(5,copy.deepcopy(changed[4]))
            for i,row in enumerate(changed):row['index']=i
            with self.assertRaises(ValueError):self.check(text,changed,data)
    def test_claimed_client_branch_id_cannot_replace_the_original_tc_issued_id(self):
        text,rows,data=fixture()
        with self.assertRaises(ValueError):self.check(text.replace('branch_id='+str(BRANCH),'branch_id='+str(BRANCH+1)),rows,data)
    def test_changed_callback_application_data_or_wrong_origin_is_rejected(self):
        text,rows,data=fixture()
        for kind in ['data','origin']:
            changed=copy.deepcopy(rows);row=next(r for r in changed if r.get('kind')==(3 if kind=='data' else 4))
            if kind=='data':row['frame_hex']=row['frame_hex'].replace(APP.encode().hex(),APP.replace('a','b').encode().hex())
            else:row['connection']=2
            with self.assertRaises(ValueError):self.check(text,changed,data)
    def test_failed_reply_cannot_claim_completion(self):
        text,rows,data=fixture();changed=copy.deepcopy(rows);row=next(r for r in changed if r.get('kind')==4);raw=bytearray.fromhex(row['frame_hex']);raw[-1]=6;row['frame_hex']=raw.hex()
        with self.assertRaises(ValueError):self.check(text,changed,data)
    def test_lock_claim_bool_or_client_counter_cannot_override_raw_evidence(self):
        text,rows,data=fixture()
        for changed in [text.replace('apply_calls=1','apply_calls=2'),text.replace('effects=1','effects=0')]:
            with self.assertRaises(ValueError):self.check(changed,rows,data)
        changed=copy.deepcopy(rows);changed[1]['delivery_count']=True
        with self.assertRaises(ValueError):self.check(text,changed,data)
    def test_journal_crc_sequence_and_completed_identity_are_checked(self):
        text,rows,data=fixture()
        for change in ['checksum','truncate','identity']:
            raw=bytearray(data)
            if change=='checksum':raw[-1]^=1
            elif change=='truncate':raw.pop()
            else:
                length=struct.unpack('>I',raw[8:12])[0];at=12+length;tail=at+4;raw[tail:tail+8]=struct.pack('>Q',3);end=len(raw)-4;raw[end:]=struct.pack('>I',zlib.crc32(raw[tail:end]))
            with self.assertRaises(ValueError):self.check(text,rows,bytes(raw))
    def test_basic_data_does_not_certify_duplicate_or_reconnect(self):
        text,rows,data=fixture()
        for case in ['duplicate-commit','reconnect-commit']:
            with self.assertRaises(ValueError):self.check(text,rows,data,case)
    def test_complete_duplicate_and_reconnect_examples_require_actual_replies(self):
        for mode in ['duplicate','reconnect']:
            result=self.check(*fault_fixture(mode),case=mode+'-commit');self.assertEqual(result['actual_RM_replies'],2)
    def test_duplicate_claim_needs_both_actual_identical_replies(self):
        text,rows,data=fault_fixture('duplicate');changed=copy.deepcopy(rows);changed.pop(next(i for i,r in enumerate(changed) if r.get('action')=='hold'))
        for i,row in enumerate(changed):row['index']=i
        with self.assertRaises(ValueError):self.check(text,changed,data,'duplicate-commit')
    def test_reconnect_reply_cannot_use_a_new_socket_without_accepted_registration(self):
        text,rows,data=fault_fixture('reconnect');changed=copy.deepcopy(rows);reply_index=next(i for i,r in enumerate(changed) if r.get('kind')==4 and r.get('connection')==2);ack_index=next(i for i,r in enumerate(changed) if r.get('kind')==104 and r.get('connection')==2)
        reply=changed.pop(reply_index);changed.insert(ack_index,reply)
        for i,row in enumerate(changed):row['index']=i
        with self.assertRaises(ValueError):self.check(text,changed,data,'reconnect-commit')
    def test_action_name_cannot_hide_suppressed_basic_reply(self):
        text,rows,data=fixture();next(row for row in rows if row.get('kind')==4)['delivery_count']=0
        with self.assertRaises(ValueError):self.check(text,rows,data)

class TemporalTests(unittest.TestCase):
    def check(self,text,rows,data,case='basic-commit'):return validator.validate(case,text,rows,NONCE,data)
    def test_final_lock_result_cannot_be_moved_before_callback_completion(self):
        text,rows,data=fixture();changed=copy.deepcopy(rows);query=changed.pop(-3);result=changed.pop(-2);at=next(i for i,r in enumerate(changed) if r.get('kind')==3);changed[at:at]=[query,result]
        for i,row in enumerate(changed):row['index']=i
        with self.assertRaises(ValueError):self.check(text,changed,data)
    def test_report_ack_cannot_be_moved_after_callback(self):
        text,rows,data=fixture();changed=copy.deepcopy(rows);report=changed.pop(next(i for i,r in enumerate(changed) if r.get('kind')==14));at=next(i for i,r in enumerate(changed) if r.get('kind')==4)+1;changed.insert(at,report)
        for i,row in enumerate(changed):row['index']=i
        with self.assertRaises(ValueError):self.check(text,changed,data)
    def test_completed_request_ids_are_not_reused_for_later_lock_queries(self):
        text,rows,data=fixture();changed=copy.deepcopy(rows)
        for row in changed:
            if row.get('kind') in [21,22]:
                raw=bytearray.fromhex(row['frame_hex']);raw[12:16]=struct.pack('>i',4);row['frame_hex']=raw.hex();row['request_id']=4
        with self.assertRaises(ValueError):self.check(text,changed,data)
    def test_final_business_cannot_return_to_closed_old_connection(self):
        text,rows,data=fault_fixture('reconnect');changed=copy.deepcopy(rows)
        for row in changed[-3:-1]:row['connection']=1
        with self.assertRaises(ValueError):self.check(text,changed,data,'reconnect-commit')
    def test_recovery_counter_cannot_claim_unobserved_recoveries(self):
        text,rows,data=fixture()
        with self.assertRaises(ValueError):self.check(text.replace('recover_calls=0','recover_calls=999'),rows,data)

if __name__=='__main__':unittest.main()
