"""Collect actual pinned Java control frames without treating missing Go merge codecs as conformance."""
import json
import hashlib
from pathlib import Path

NAMES=['HeartbeatPing','HeartbeatPong','HeartbeatHeadMap','MergeRequestWithIds','MergeRequestWithoutIds','MergeRequestEmpty','MergeRequestNested','MergeResult','MergeResultEmpty']

def validate_rows(rows):
    if len(rows)!=9 or {r.get('name') for r in rows}!=set(NAMES):raise ValueError('Incomplete/duplicate control capture inventory')
    for row in rows:
        frame=bytes.fromhex(row['frame_hex']);payload=bytes.fromhex(row['serialized_hex']);name=row['name']
        if len(frame)<16 or frame[:3]!=b'\xda\xda\x01' or int.from_bytes(frame[3:7],'big')!=len(frame):raise ValueError('Invalid original complete frame')
        head=int.from_bytes(frame[7:9],'big');kind=4 if name=='HeartbeatPong' else (3 if name.startswith('Heartbeat') else (1 if name.startswith('MergeResult') else 0))
        if not 16<=head<=len(frame) or frame[9:12]!=bytes([kind,1,0]) or row['kind']!=kind or row['request_id']!=int.from_bytes(frame[12:16],'big'):raise ValueError('Outer correlation/header differs')
        if frame[head:]!=payload:raise ValueError('Captured payload does not match frame body')
        if name.startswith('Heartbeat'):
            if payload:raise ValueError('Heartbeat must be bodyless')
        elif payload[:2]!=(60 if name.startswith('MergeResult') else 59).to_bytes(2,'big') or len(payload)<8 or int.from_bytes(payload[2:6],'big')!=len(payload)-6:raise ValueError('Invalid sealed Java merge payload')
    return sorted(rows,key=lambda r:NAMES.index(r['name']))

def read_rows(log):
    rows=[json.loads(line.split(b'SEATA_CONTROL_WIRE ',1)[1]) for line in Path(log).read_bytes().splitlines() if line.startswith(b'SEATA_CONTROL_WIRE ')]
    return validate_rows(rows)

def export(rows,destination):
    rows=validate_rows(rows);destination=Path(destination);destination.mkdir(parents=True,exist_ok=False)
    for row in rows:
        (destination/(row['name']+'.frame.bin')).write_bytes(bytes.fromhex(row['frame_hex']))
        if row['serialized_hex']:(destination/(row['name']+'.payload.bin')).write_bytes(bytes.fromhex(row['serialized_hex']))
    (destination/'vectors.json').write_text(json.dumps(rows,indent=2)+'\n',encoding='utf-8')
    (destination/'vectors.tsv').write_text(''.join(f"{r['name']}\t{r['frame_hex']}\t{r['serialized_hex']}\n" for r in rows),encoding='utf-8')
    files=[dict(path=p.name,sha256=hashlib.sha256(p.read_bytes()).hexdigest()) for p in sorted(destination.iterdir()) if p.is_file()]
    (destination/'manifest.json').write_text(json.dumps(dict(java_pin='cba73e180fd8896a99df6415b859d275122feaa7',go_pin='82570d905919c1c66bb025a3a5979f25390de498',scope='Java-generated control candidates; Go/Rust execution and classifications recorded separately',files=files),indent=2)+'\n',encoding='utf-8')
