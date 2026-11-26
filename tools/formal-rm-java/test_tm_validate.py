"""Independent TM raw records/TC log binding; synthetic examples are verifier tests only."""
import copy
import unittest
import test_validate as rm


def fixture(commit=True):
    text,rows,journal=rm.fixture(commit)
    for i,row in enumerate(rows):row['monotonic_ns']=(100+i)*1_000_000
    clock=dict(name='perf_counter_ns',implementation='synthetic verifier fixture',resolution_seconds=1e-7,monotonic=True,adjustable=False)
    rows[0]['clock']=clock
    values=[dict(event='ready',index=0,role='TM',mode='basic',clock=clock.copy())]
    def add(direction,kind,wire,body,nanos):
        typ=0 if direction=='c2s' else 1;raw=rm.frame(kind,typ,wire,body)
        values.append(dict(event='frame',index=len(values),role='TM',direction=direction,connection=1,kind=kind,request_type=typ,request_id=wire,frame_hex=raw.hex(),action='forward',delivery_count=1,monotonic_ns=nanos))
    add('c2s',101,1,rm.string('1.1.0')+rm.string('phase0-auth')+rm.string('default_tx_group')+rm.string('phase0-local-only'),10_000_000)
    add('s2c',102,1,b'\x01'+rm.string(None),11_000_000)
    for wire,xid,suffix,when in [(2,rm.XID,'resource',12_000_000),(3,rm.CONTENDER,'contender',14_000_000)]:
        add('c2s',1,wire,(60000).to_bytes(4,'big')+rm.string('rm-'+rm.NONCE+'-'+suffix),when)
        add('s2c',2,wire,b'\x01\x00'+rm.string(xid)+rm.string(None),when+1_000_000)
    kind=7 if commit else 9;status=8 if commit else 11
    add('c2s',kind,4,rm.string(rm.XID)+rm.string(None),110_500_000)
    add('s2c',kind+1,4,b'\x01\x00'+bytes([status]),110_600_000 if commit else 112_050_000)
    add('c2s',15,5,rm.string(rm.XID)+rm.string(None),112_100_000);add('s2c',16,5,b'\x01\x00\x0f',112_200_000)
    add('c2s',9,6,rm.string(rm.CONTENDER)+rm.string(None),114_100_000);add('s2c',10,6,b'\x01\x00\x0b',114_200_000)
    add('c2s',15,7,rm.string(rm.CONTENDER)+rm.string(None),114_300_000);add('s2c',16,7,b'\x01\x00\x0f',114_400_000)
    values.append(dict(event='summary',index=len(values),role='TM',mode='basic',succeeded=True,connections=1,faults=0,released=0,closed=0,errors=[]))
    log=[]
    for suffix,xid in [('resource',rm.XID),('contender',rm.CONTENDER)]:
        name='rm-'+rm.NONCE+'-'+suffix
        log.append(f"receive msg[single]: GlobalBeginRequest{{transactionName='{name}', timeout=60000}}")
        log.append(f'Begin new global transaction applicationId: phase0-auth,transactionServiceGroup: default_tx_group, transactionName: {name},timeout:60000,xid:{xid}')
    for operation,xid in [('Commit' if commit else 'Rollback',rm.XID),('Rollback',rm.CONTENDER)]:log.append(f"receive msg[single]: Global{operation}Request{{xid='{xid}', extraData='null'}}")
    before,after=text.split('PASS ',1);text=before+f'OBSERVE xid={rm.XID} status=15\nOBSERVE xid={rm.CONTENDER} status=15\nPASS '+after
    result=rm.validator.validate('basic-'+('commit' if commit else 'rollback'),text,rows,rm.NONCE,journal)
    return result,text,values,rows,'\n'.join(log).encode()


class TmBindingTests(unittest.TestCase):
    def check(self,parts):return rm.validator.validate_tm(*parts,rm.NONCE)
    def test_raw_tm_and_tc_log_agree_with_the_original_two_transactions(self):
        for commit in [True,False]:self.assertEqual(self.check(fixture(commit))['global_begins'],2)
    def test_end_status_cannot_be_changed_to_another_allowed_domain_value(self):
        values=list(fixture());values[1]=values[1].replace('status=8 unknown=false','status=15 unknown=false')
        with self.assertRaises(ValueError):self.check(values)
    def test_observed_finished_cannot_replace_an_actual_in_progress_raw_status(self):
        values=list(fixture());values[2]=copy.deepcopy(values[2]);row=next(r for r in values[2] if r.get('kind')==16);raw=bytearray.fromhex(row['frame_hex']);raw[-1]=8;row['frame_hex']=raw.hex()
        with self.assertRaises(ValueError):self.check(values)
    def test_original_end_must_precede_the_actual_rm_callback(self):
        values=list(fixture());values[2]=copy.deepcopy(values[2]);next(r for r in values[2] if r.get('kind')==7)['monotonic_ns']=113_000_000
        with self.assertRaises(ValueError):self.check(values)
    def test_extra_begin_or_opposite_end_in_original_tc_log_is_rejected(self):
        for suffix in [b"\nreceive msg[single]: GlobalBeginRequest{transactionName='unexpected', timeout=60000}",b"\nreceive msg[single]: GlobalRollbackRequest{xid='"+rm.XID.encode()+b"', extraData='null'}"]:
            values=list(fixture());values[-1]+=suffix
            with self.assertRaises(ValueError):self.check(values)
    def test_tc_actual_xid_cannot_be_replaced_by_only_a_matching_driver_claim(self):
        values=list(fixture());values[-1]=values[-1].replace(('xid:'+rm.XID).encode(),b'xid:changed')
        with self.assertRaises(ValueError):self.check(values)
    def test_low_resolution_capture_cannot_claim_strict_cross_process_order(self):
        values=list(fixture());values[2]=copy.deepcopy(values[2]);values[2][0]['clock']['resolution_seconds']=0.015625
        with self.assertRaises(ValueError):self.check(values)
    def test_contender_rollback_cannot_be_mislabeled_committed(self):
        values=list(fixture());values[2]=copy.deepcopy(values[2]);row=next(r for r in values[2] if r.get('kind')==10);raw=bytearray.fromhex(row['frame_hex']);raw[-1]=9;row['frame_hex']=raw.hex()
        with self.assertRaises(ValueError):self.check(values)
    def test_late_failed_target_end_is_kept_separate_from_the_original_unknown(self):
        values=list(fixture(False));values[1]=values[1].replace('status=11 unknown=false','status=unknown unknown=true');values[2]=copy.deepcopy(values[2])
        row=next(r for r in values[2] if r.get('kind')==10);row['frame_hex']=rm.frame(10,1,row['request_id'],b'\x00'+rm.string('late target failure')+b'\x00\x05').hex()
        proof=self.check(values);self.assertTrue(proof['client_reported_end_unknown']);self.assertEqual(proof['target_end_result_code'],0);self.assertEqual(proof['target_end_response_status'],5)

if __name__=='__main__':unittest.main()
