import io
import socket
import struct
import threading
import unittest

try:
    import loss_proxy
except ImportError:
    loss_proxy = None


def frame(kind, request_id, body=b'', response=False):
    payload = struct.pack('>H',kind) + body
    return b'\xda\xda\x01'+struct.pack('>IHBBBI',16+len(payload),16,int(response),1,0,request_id)+payload


class ResponseLossProxyTest(unittest.TestCase):
    def setUp(self): self.assertIsNotNone(loss_proxy, 'External loss proxy missing')

    def test_frame_reader_preserves_complete_frames_and_rejects_truncation(self):
        raw = frame(12,77,b'\x01\x00'+struct.pack('>q',42),True)
        self.assertEqual(loss_proxy.read_frame(io.BytesIO(raw)),raw)
        self.assertIsNone(loss_proxy.read_frame(io.BytesIO(b'')))
        for end in range(1,len(raw)):
            with self.assertRaises((EOFError,ValueError)): loss_proxy.read_frame(io.BytesIO(raw[:end]))
        for changed in [b'xx'+raw[2:],raw[:3]+struct.pack('>I',8*1024*1024+1)+raw[7:],raw[:7]+b'\x00\x0f'+raw[9:]]:
            with self.assertRaises(ValueError): loss_proxy.read_frame(io.BytesIO(changed))

    def test_branch_register_drop_requires_correlated_success_and_occurs_once(self):
        rule = loss_proxy.DropRule(12)
        rule.inspect('c2s',0,frame(103,2))
        rule.inspect('c2s',0,frame(11,77,b'\x00\x01x'))
        self.assertFalse(rule.inspect('s2c',0,frame(12,78,b'\x01\x00'+struct.pack('>q',42),True))['dropped'])
        self.assertFalse(rule.inspect('s2c',0,frame(12,77,b'\x00\x00\x00\x02'+b'\x00'*8,True))['dropped'])
        self.assertFalse(rule.inspect('s2c',0,frame(12,77,b'\x01\x00'+b'\x00'*8,True))['dropped'])
        success = frame(12,77,b'\x01\x00'+struct.pack('>q',42),True)
        dropped = rule.inspect('s2c',0,success)
        self.assertTrue(dropped['dropped']); self.assertEqual(dropped['xid'],'x'); self.assertEqual(dropped['branch_id'],42)
        self.assertFalse(rule.inspect('s2c',0,success)['dropped']); self.assertEqual(rule.drop_count,1)

    def test_end_drop_matches_expected_status_and_connection(self):
        for response,request,status in [(8,7,9),(10,9,11)]:
            rule=loss_proxy.DropRule(response)
            rule.inspect('c2s',0,frame(101,1))
            rule.inspect('c2s',0,frame(request,77,b'\x00\x01x\x00\x00'))
            raw=frame(response,77,bytes([1,0,status]),True)
            self.assertFalse(rule.inspect('s2c',1,raw)['dropped'])
            self.assertFalse(rule.inspect('s2c',0,frame(response,77,b'\x01\x00\x05',True))['dropped'])
            self.assertTrue(rule.inspect('s2c',0,raw)['dropped'])
        with self.assertRaises(ValueError): loss_proxy.DropRule(2)

    def test_callback_requests_and_responses_are_forwarded_without_fault_injection(self):
        rule=loss_proxy.DropRule(10)
        callback=rule.inspect('s2c',0,frame(5,99,b'\x00\x01x'+struct.pack('>q',42)+b'\x00\x00\x01r\x00\x00\x00\x02{}'))
        reply=rule.inspect('c2s',0,frame(6,99,b'\x01\x00\x00\x01x'+struct.pack('>q',42)+b'\x08',True))
        self.assertFalse(callback['dropped']);self.assertFalse(reply['dropped'])
        self.assertEqual((callback['xid'],callback['branch_id']),('x',42));self.assertEqual((reply['xid'],reply['branch_id']),('x',42))
        self.assertEqual(rule.drop_count,0)

    def test_external_tcp_path_actually_withholds_response_and_stops_cleanly(self):
        upstream=socket.socket();upstream.bind(('127.0.0.1',0));upstream.listen(2);upstream.settimeout(3)
        errors=[];workers=[]
        def handle(channel):
            try:
                with channel:
                    channel.settimeout(3)
                    first=loss_proxy.read_frame(channel);kind=struct.unpack('>H',first[16:18])[0]
                    channel.sendall(frame(kind+1,struct.unpack('>I',first[12:16])[0],b'\x01\x00\x01v',True))
                    if kind==103:
                        req=loss_proxy.read_frame(channel)
                        channel.sendall(frame(12,struct.unpack('>I',req[12:16])[0],b'\x01\x00'+struct.pack('>q',42),True))
                    while loss_proxy.read_frame(channel) is not None: pass
            except Exception as error: errors.append(error)
        def accept():
            try:
                for _ in range(2):
                    channel,_=upstream.accept();thread=threading.Thread(target=handle,args=(channel,),daemon=True);thread.start();workers.append(thread)
            except Exception as error: errors.append(error)
            finally: upstream.close()
        acceptor=threading.Thread(target=accept,daemon=True);acceptor.start()
        sink=loss_proxy.EventSink()
        proxy=loss_proxy.ProxyServer(0,upstream.getsockname()[1],12,sink,io_timeout=3)
        proxy_errors=[]
        def run():
            try: proxy.serve()
            except Exception as error: proxy_errors.append(error)
        running=threading.Thread(target=run,daemon=True);running.start()
        tm=socket.create_connection(('127.0.0.1',proxy.port));rm=None
        try:
            tm.sendall(frame(101,1));self.assertIsNotNone(loss_proxy.read_frame(tm))
            rm=socket.create_connection(('127.0.0.1',proxy.port));rm.sendall(frame(103,2));self.assertIsNotNone(loss_proxy.read_frame(rm))
            rm.sendall(frame(11,77,b'\x00\x01x'));rm.settimeout(.25)
            with self.assertRaises(socket.timeout): loss_proxy.read_frame(rm)
        finally:
            tm.close()
            if rm is not None: rm.close()
        running.join(4);acceptor.join(4)
        for thread in workers: thread.join(4)
        self.assertFalse(running.is_alive());self.assertFalse(errors);self.assertFalse(proxy_errors)
        summary=[row for row in sink.rows if row['event']=='summary']
        self.assertEqual(len(summary),1);self.assertTrue(summary[0]['succeeded']);self.assertEqual(summary[0]['drop_count'],1)


if __name__=='__main__': unittest.main()
