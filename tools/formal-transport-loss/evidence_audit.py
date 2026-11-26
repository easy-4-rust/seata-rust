"""父门禁额外复核：将客户端状态声明和锁状态顺序绑定到真实原始字节。"""
import re
from live import validate as validate_live, validate_wire_rows


def validate(mode, text, rows, nonce):
    result = validate_live(mode, text, rows, nonce)
    rows = validate_wire_rows(rows)
    if mode == 'begin':
        return result
    target = result['xid']
    frames = [(i, row) for i, row in enumerate(rows) if row['event'] == 'frame']
    requests = [(i, row) for i, row in frames if row['direction'] == 'c2s' and row['request_type'] == 0]
    end_kind = 7 if mode == 'commit' else 9
    endings = [(i, row) for i, row in requests if row['kind'] == end_kind and row['xid'] == target]
    first_end = endings[0][0]
    before = [row for i, row in frames if i < first_end and row['kind'] == 16 and row['xid'] == target]
    if not before or before[-1]['global_status'] != 1:
        raise ValueError('Original XID was not observed in Begin before explicit end')
    callbacks = [(i, row) for i, row in frames if row['direction'] == 's2c' and row['kind'] == (3 if mode == 'commit' else 5)]
    replies = [(i, row) for i, row in frames if row['direction'] == 'c2s' and row['kind'] == (4 if mode == 'commit' else 6)]
    locks = [(i, row) for i, row in frames if row['direction'] == 's2c' and row['kind'] == 22]
    queries = [(i, row) for i, row in requests if row['kind'] == 21]
    if not locks[0][0] < first_end < callbacks[0][0] < replies[0][0] < queries[1][0]:
        raise ValueError('Lock/end/callback/release evidence order differs')
    contender = queries[0][1]['xid']
    cleanup = [(i, row) for i, row in frames if row['direction'] == 's2c' and row['kind'] == 10 and row['xid'] == contender]
    if len(cleanup) != 1 or cleanup[0][0] <= locks[1][0] or cleanup[0][1]['global_status'] != 11:
        raise ValueError('Lock contender cleanup was not explicitly acknowledged')
    if mode == 'branch-register':
        ends = [row for i, row in frames if row['direction'] == 's2c' and row['kind'] == 10 and row['xid'] == target and row['matched_request_index'] == first_end and i > replies[0][0]]
        if len(ends) != 1 or ends[0]['global_status'] != 11:
            raise ValueError('Pending branch cancellation was not explicitly Rollbacked')
        return result
    dropped = [row for _, row in frames if row['dropped']][0]
    if dropped['global_status'] != 9 or dropped['matched_request_index'] != first_end:
        raise ValueError('Original dropped Commit was not a successful Committed reply')
    second_end = endings[1][0]
    observations = [row for i, row in frames if first_end < i < second_end and row['kind'] == 16 and row['xid'] == target]
    retry = [row for _, row in frames if row['kind'] == 8 and row['matched_request_index'] == second_end]
    if not observations or len(retry) != 1 or replies[0][0] >= second_end:
        raise ValueError('Commit duplicate preceded authoritative status/callback reconciliation')
    claim = re.search(r'observed=(\d+) retry_status=(\d+) confirmed=(true|false) actions=1', text)
    confirmed = any(row['global_status'] == 9 for row in observations + retry)
    if not claim or (int(claim[1]), int(claim[2]), claim[3] == 'true') != (observations[-1]['global_status'], retry[0]['global_status'], confirmed):
        raise ValueError('Commit status/confirmation claim differs from actual delivered responses')
    if observations[-1]['global_status'] not in [9, 15] or retry[0]['global_status'] not in [9, 15]:
        raise ValueError('Commit reconciliation did not reach bounded fixture terminal status')
    return result
