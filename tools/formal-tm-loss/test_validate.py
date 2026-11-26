"""手工V1字节与破坏控制，独立于Rust编码器，验证正式TM故障验收器。"""
import copy
import importlib.util
from pathlib import Path
import struct
import unittest
from validate import validate

ROOT=Path(__file__).resolve().parents[2]
spec=importlib.util.spec_from_file_location('tm_loss_proxy_test',ROOT/'tools/formal-transport-loss/loss_proxy.py')
proxy=importlib.util.module_from_spec(spec);spec.loader.exec_module(proxy)
TARGET='tc:9007199254740993';CONTENDER='contender';BRANCH=9007199254740995;CALLBACK=-2147483648
RESOURCE='mysql://auth-a';LOCK='orders:1';DATA='{"formalOperation":"n"}'
def s16(value):
    data=value.encode();return struct.pack('>h',len(data))+data
def s32(value):
    data=value.encode();return struct.pack('>i',len(data))+data
def frame(kind,body,wire_id,reply=False):
    payload=struct.pack('>H',kind)+body
    return b'\xda\xda\x01'+struct.pack('>IHBBB',16+len(payload),16,int(reply),1,0)+struct.pack('>i',wire_id)+payload


def fixture(mode,known=False):
    rule=proxy.DropRule({'begin':2,'commit':8,'rollback':10}[mode]);rows=[]
    def add(direction,connection,kind,body,wire_id,reply=False):
        rows.append(rule.inspect(direction,connection,frame(kind,body,wire_id,reply)))
    auth=s16('1.1.0')+s16('phase0-auth')+s16('default_tx_group')+s16('phase0-local-only')
    add('c2s',0,101,auth,1);add('s2c',0,102,b'\x01\x00\x00',1,True)
    add('c2s',1,103,auth+s32(RESOURCE),1);add('s2c',1,104,b'\x01\x00\x00',1,True)
    if mode=='begin':
        add('c2s',0,1,struct.pack('>i',60000)+s16('tm-loss-n-begin'),2)
        add('s2c',0,2,b'\x01\x00'+s16(TARGET)+s16(''),2,True)
        text='PASS TM_LOSS_BEGIN operation=1 request_id=2 xid_known=false begin_attempts=1 business_started=false unknown=true\n'
    else:
        commit=mode=='commit';operation=7 if commit else 9;terminal=9 if commit else 11
        for wire_id,xid,suffix in [(2,CONTENDER,'contender'),(3,TARGET,'target')]:
            add('c2s',0,1,struct.pack('>i',60000)+s16('tm-loss-n-'+suffix),wire_id)
            add('s2c',0,2,b'\x01\x00'+s16(xid)+s16(''),wire_id,True)
        add('c2s',1,11,s16(TARGET)+b'\x00'+s16(RESOURCE)+s32(LOCK)+s32(DATA),2)
        add('s2c',1,12,b'\x01\x00'+struct.pack('>q',BRANCH),2,True)
        add('c2s',0,15,s16(TARGET)+s16(''),4);add('s2c',0,16,b'\x01\x00\x01',4,True)
        add('c2s',1,21,s16(CONTENDER)+b'\x00'+s16(RESOURCE)+s32(LOCK)+s32(''),3)
        add('s2c',1,22,b'\x01\x00\x00\x00',3,True)
        add('c2s',0,operation,s16(TARGET)+s16(''),5)
        add('s2c',1,3 if commit else 5,s16(TARGET)+struct.pack('>q',BRANCH)+b'\x00'+s16(RESOURCE)+s32(DATA),CALLBACK)
        add('c2s',1,4 if commit else 6,b'\x01\x00'+s16(TARGET)+struct.pack('>q',BRANCH)+bytes([5 if commit else 8]),CALLBACK,True)
        add('s2c',0,operation+1,b'\x01\x00'+bytes([terminal]),5,True)
        add('c2s',0,15,s16(TARGET)+s16(''),6);add('s2c',0,16,b'\x01\x00'+bytes([terminal if known else 15]),6,True)
        if not known:
            add('c2s',0,operation,s16(TARGET)+s16(''),7);add('s2c',0,operation+1,b'\x01\x00\x0f',7,True)
        add('c2s',1,21,s16(CONTENDER)+b'\x00'+s16(RESOURCE)+s32(LOCK)+s32(''),4)
        add('s2c',1,22,b'\x01\x00\x00\x01',4,True)
        add('c2s',0,9,s16(CONTENDER)+s16(''),8);add('s2c',0,10,b'\x01\x00\x0b',8,True)
        text=f'PASS TM_LOSS_{mode.upper()} operation={operation} request_id=5 xid={TARGET} branch_id={BRANCH} callback_id={CALLBACK} attempts={1 if known else 2} explicit_retry={str(not known).lower()} observed={terminal if known else 15} retry_status={-1 if known else 15} unknown_retained={str(not known).lower()} actions=1 business_error_preserved={str(not commit).lower()}\n'
    rows.append(dict(event='summary',succeeded=True,drop_count=rule.drop_count,roles=['RM','TM'],errors=[]))
    return rows,text


class ValidatorControls(unittest.TestCase):
    def test_positive_cases_preserve_exact_branch_id_and_status_dependent_recovery(self):
        for mode,known in [('begin',False),('commit',False),('rollback',False),('commit',True),('rollback',True)]:
            rows,text=fixture(mode,known);value=validate(mode,text,rows,'n');self.assertEqual(value['drop_count'],1)
            if mode!='begin':self.assertEqual(value['branch_id'],BRANCH);self.assertEqual(value['unknown_retained'],not known)
    def test_begin_does_not_expose_observer_xid_or_construct_business(self):
        rows,text=fixture('begin')
        for changed in [text.replace('business_started=false','business_started=true'),text+TARGET]:
            with self.assertRaises(ValueError):validate('begin',changed,rows,'n')
    def test_blind_second_begin_is_rejected(self):
        rows,text=fixture('begin');extra=copy.deepcopy(rows[4]);raw=bytearray.fromhex(extra['frame_hex']);raw[12:16]=struct.pack('>i',99);extra.update(request_id=99,frame_hex=raw.hex());rows.insert(-1,extra)
        with self.assertRaises(ValueError):validate('begin',text,rows,'n')
    def test_retry_cannot_switch_the_original_operation(self):
        rows,text=fixture('commit')
        for row in rows:
            if row.get('request_id')==7 and row.get('kind') in [7,8]:
                row['kind']+=2;raw=bytearray.fromhex(row['frame_hex']);raw[16:18]=struct.pack('>H',row['kind']);row['frame_hex']=raw.hex()
        with self.assertRaises(ValueError):validate('commit',text,rows,'n')
    def test_retry_requires_actual_status_between_original_and_duplicate(self):
        rows,text=fixture('rollback');rows=[row for row in rows if row.get('request_id')!=6]
        with self.assertRaises(ValueError):validate('rollback',text,rows,'n')
    def test_finished_cannot_be_rewritten_as_confirmed_commit_in_client_claim(self):
        rows,text=fixture('commit')
        for changed in [text.replace('observed=15','observed=9').replace('unknown_retained=true','unknown_retained=false'),text.replace('unknown_retained=true','unknown_retained=false')]:
            with self.assertRaises(ValueError):validate('commit',changed,rows,'n')
    def test_raw_reply_identity_and_registration_branch_are_bound(self):
        for kind in [4,12]:
            rows,text=fixture('commit');row=next(row for row in rows if row.get('kind')==kind);raw=bytearray.fromhex(row['frame_hex']);raw[-2 if kind==4 else -1]^=1;row['frame_hex']=raw.hex()
            if kind==12:row['branch_id']^=1
            with self.assertRaises(ValueError):validate('commit',text,rows,'n')
    def test_callback_nonce_and_original_connection_cannot_be_substituted(self):
        for field in ['nonce','connection']:
            rows,text=fixture('rollback')
            for row in rows:
                if row.get('kind') in [5,6]:
                    if field=='connection':row['connection']=42
                    elif row['kind']==5:row['frame_hex']=bytes.fromhex(row['frame_hex']).replace(b'"n"',b'"m"').hex()
            with self.assertRaises(ValueError):validate('rollback',text,rows,'n')
    def test_lock_occupation_and_release_use_original_resource_and_key(self):
        for old,new in [(b'mysql://auth-a',b'mysql://auth-b'),(b'orders:1',b'orders:2')]:
            rows,text=fixture('commit');row=next(row for row in rows if row.get('kind')==21);row['frame_hex']=bytes.fromhex(row['frame_hex']).replace(old,new).hex()
            with self.assertRaises(ValueError):validate('commit',text,rows,'n')
    def test_only_successful_original_end_response_can_be_withheld(self):
        rows,text=fixture('rollback');row=next(row for row in rows if row.get('dropped'));raw=bytearray.fromhex(row['frame_hex']);raw[-1]=15;row.update(frame_hex=raw.hex(),global_status=15)
        with self.assertRaises(ValueError):validate('rollback',text,rows,'n')
    def test_rollback_keeps_business_error_and_native_identity_must_be_accepted(self):
        rows,text=fixture('rollback')
        with self.assertRaises(ValueError):validate('rollback',text.replace('business_error_preserved=true','business_error_preserved=false'),rows,'n')
        row=next(row for row in rows if row.get('kind')==104);raw=bytearray.fromhex(row['frame_hex']);raw[18]=0;row['frame_hex']=raw.hex()
        with self.assertRaises(ValueError):validate('rollback',text,rows,'n')

    def test_original_response_is_unique_and_not_also_forwarded(self):
        for mode in ['begin','commit','rollback']:
            with self.subTest(mode=mode):
                rows,text=fixture(mode);index=next(i for i,row in enumerate(rows) if row.get('dropped'))
                forwarded=copy.deepcopy(rows[index]);forwarded['dropped']=False;rows.insert(index+1,forwarded)
                with self.assertRaises(ValueError):validate(mode,text,rows,'n')

    def test_inflight_status_request_cannot_be_used_as_post_loss_reconciliation(self):
        for mode in ['commit','rollback']:
            with self.subTest(mode=mode):
                rows,text=fixture(mode);index=next(i for i,row in enumerate(rows) if row.get('kind')==15 and row['request_id']==6)
                request=rows.pop(index);first_end=next(i for i,row in enumerate(rows) if row.get('kind')==(7 if mode=='commit' else 9));rows.insert(first_end,request)
                with self.assertRaises(ValueError):validate(mode,text,rows,'n')


if __name__=='__main__':unittest.main()
