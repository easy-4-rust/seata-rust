"""Owned loopback TLS 1.3 mutual-auth frontends; original TC observers stay unchanged."""
import asyncio
import base64
import hashlib
import ipaddress
from pathlib import Path
import ssl
import threading
import time


class Frontend:
    def __init__(self, material, upstream, role):
        host, port = upstream.rsplit(':', 1)
        if not ipaddress.ip_address(host).is_loopback or not 0 < int(port) < 65536:
            raise ValueError('TLS fixture backend must remain loopback')
        self.upstream = host, int(port)
        self.role = role
        self.events = []
        self.handshake_errors = []
        self.port = None
        self.error = None
        self.ready = threading.Event()
        self.loop = None
        self.stop_event = None
        self.writers = set()
        self.handlers = set()
        material = Path(material)
        self.context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
        self.context.minimum_version = ssl.TLSVersion.TLSv1_3
        self.context.maximum_version = ssl.TLSVersion.TLSv1_3
        self.context.verify_mode = ssl.CERT_REQUIRED
        self.context.load_verify_locations(cadata=ssl.DER_cert_to_PEM_cert((material / 'root.der').read_bytes()))
        cert = material / 'server.pem'
        key = material / 'server.key.pem'
        cert.write_text(ssl.DER_cert_to_PEM_cert((material / 'server.der').read_bytes()), encoding='ascii')
        key.write_text('-----BEGIN PRIVATE KEY-----\n' + '\n'.join(
            base64.b64encode((material / 'server.key.der').read_bytes()).decode()[i:i+64]
            for i in range(0, len(base64.b64encode((material / 'server.key.der').read_bytes())), 64)
        ) + '\n-----END PRIVATE KEY-----\n', encoding='ascii')
        self.context.load_cert_chain(cert, key)
        self.expected_client = hashlib.sha256((material / 'client.der').read_bytes()).hexdigest()
        self.thread = threading.Thread(target=self._thread, daemon=True)
        self.thread.start()
        if not self.ready.wait(10) or self.error:
            raise RuntimeError('Owned TLS frontend did not start')

    def _thread(self):
        try:
            asyncio.run(self._main())
        except Exception as error:
            self.error = type(error).__name__
            self.ready.set()

    async def _main(self):
        self.loop = asyncio.get_running_loop()
        self.loop.set_debug(True)
        def handshake_error(loop, context):
            error = context.get('exception')
            self.handshake_errors.append(dict(kind=type(error).__name__, reason=getattr(error, 'reason', None), verify_code=getattr(error, 'verify_code', None)))
        self.loop.set_exception_handler(handshake_error)
        self.stop_event = asyncio.Event()
        server = await asyncio.start_server(self._handle, '127.0.0.1', 0, ssl=self.context,
                                            ssl_handshake_timeout=3, ssl_shutdown_timeout=1)
        self.port = server.sockets[0].getsockname()[1]
        self.ready.set()
        await self.stop_event.wait()
        server.close()
        await server.wait_closed()
        for writer in list(self.writers):
            writer.close()
        for task in list(self.handlers):
            task.cancel()
        await asyncio.gather(*list(self.handlers), return_exceptions=True)

    async def _handle(self, reader, writer):
        task = asyncio.current_task()
        self.handlers.add(task)
        self.writers.add(writer)
        session = writer.get_extra_info('ssl_object')
        row = dict(role=self.role, connection=len(self.events)+1, TLS=session.version(),
                   peer_sha256=hashlib.sha256(session.getpeercert(binary_form=True)).hexdigest(),
                   upstream_bytes=0, downstream_bytes=0, opened_qpc_ns=time.perf_counter_ns(), closed=False)
        self.events.append(row)
        backend_writer = None
        children = []
        try:
            if row['TLS'] != 'TLSv1.3' or row['peer_sha256'] != self.expected_client:
                raise ValueError('Authenticated peer identity differs')
            backend_reader, backend_writer = await asyncio.wait_for(asyncio.open_connection(*self.upstream), 3)
            self.writers.add(backend_writer)

            async def copy(source, destination, counter):
                while data := await source.read(16384):
                    destination.write(data)
                    await destination.drain()
                    row[counter] += len(data)

            children = [asyncio.create_task(copy(reader, backend_writer, 'upstream_bytes')),
                        asyncio.create_task(copy(backend_reader, writer, 'downstream_bytes'))]
            done, _ = await asyncio.wait(children, return_when=asyncio.FIRST_COMPLETED)
            for child in done:
                child.result()
        except (ConnectionError, ssl.SSLError, asyncio.CancelledError):
            row['termination'] = 'closed'
        except Exception as error:
            self.error = type(error).__name__
            row['termination'] = 'fixture_error'
        finally:
            for child in children:
                child.cancel()
            await asyncio.gather(*children, return_exceptions=True)
            for current in [writer, backend_writer]:
                if current:
                    current.close()
                    self.writers.discard(current)
                    try:
                        await asyncio.wait_for(current.wait_closed(), 2)
                    except (ConnectionError, ssl.SSLError, TimeoutError):
                        pass
            row['closed'] = True
            row['closed_qpc_ns'] = time.perf_counter_ns()
            self.handlers.discard(task)

    def stop(self):
        self.loop.call_soon_threadsafe(self.stop_event.set)
        self.thread.join(10)
        if self.thread.is_alive() or self.error:
            raise RuntimeError('Owned TLS frontend teardown failed')
        return dict(role=self.role, mutual_TLS=True, TLS='TLSv1.3', backend='loopback-only',
                    expected_client_sha256=self.expected_client, sessions=self.events,
                    handshake_errors=self.handshake_errors, stopped=True, active_handlers=len(self.handlers), active_writers=len(self.writers))
