"""独立预期核对实际 JVM/Go 观察；不推导运行时转换可达性。"""
import struct
from urllib.parse import unquote, urlparse

NAMES = ['UnKnown', 'Begin', 'Committing', 'CommitRetrying', 'Rollbacking',
         'RollbackRetrying', 'TimeoutRollbacking', 'TimeoutRollbackRetrying',
         'AsyncCommitting', 'Committed', 'CommitFailed', 'Rollbacked',
         'RollbackFailed', 'TimeoutRollbacked', 'TimeoutRollbackFailed', 'Finished',
         'CommitRetryTimeout', 'RollbackRetryTimeout', 'Deleting',
         'StopCommitOrCommitRetry', 'StopRollbackOrRollbackRetry']
GROUPS = {
    'one_phase_timeout': {6, 7, 13, 14},
    'two_phase_success': {9, 11, 13, 18},
    'two_phase_heuristic': {15},
    'one_phase_prepare_failed': {5},
    'timeout_global': {6, 7, 13, 14},
    'timeout_rollbacking': {6, 7},
    'rollback_global': {4, 5, 11, 12, 17},
    'end_global': {9, 11, 13, 15},
    'commit_global': {2, 3, 8, 9, 10, 16},
}
TYPES = {8, 10, 16, 18}
EXPECTED_CLASSES = {
    'org.apache.seata.core.model.GlobalStatus',
    'org.apache.seata.server.session.SessionStatusValidator',
    'org.apache.seata.serializer.seata.SeataSerializer',
    'org.apache.seata.serializer.seata.SeataSerializer$SeataSerializerV1',
    'org.apache.seata.serializer.seata.MessageCodecFactory',
    'org.apache.seata.serializer.seata.protocol.AbstractResultMessageCodec',
    'org.apache.seata.serializer.seata.protocol.transaction.AbstractTransactionResponseCodec',
    'org.apache.seata.serializer.seata.protocol.transaction.AbstractGlobalEndResponseCodec',
} | {'org.apache.seata.core.protocol.transaction.' + name for name in
     ['GlobalCommitResponse', 'GlobalRollbackResponse', 'GlobalStatusResponse', 'GlobalReportResponse']} | {
    'org.apache.seata.serializer.seata.protocol.transaction.' + name + 'Codec' for name in
    ['GlobalCommitResponse', 'GlobalRollbackResponse', 'GlobalStatusResponse', 'GlobalReportResponse']}


def check_origins(rows):
    require(isinstance(rows, list) and len(rows) == len(EXPECTED_CLASSES), 'Need exact critical class set')
    names = [row.get('class') for row in rows]
    require(len(set(names)) == len(names) and set(names) == EXPECTED_CLASSES, 'Critical classes differ or repeat')
    for row in rows:
        location = urlparse(row.get('location', ''))
        path = unquote(location.path).lstrip('/')
        require(location.scheme == 'file' and not location.netloc and path[:3].upper() in ['D:/', 'E:/'], 'Unapproved class location')


def require(condition, message):
    if not condition:
        raise ValueError(message)


def index(rows, keys, expected):
    require(isinstance(rows, list) and len(rows) == len(expected), 'Wrong row count')
    result = {}
    for row in rows:
        require(isinstance(row, dict), 'Row must be an object')
        require(all(type(row.get(key)) is int for key in keys), 'Identity must contain integers')
        identity = tuple(row[key] for key in keys)
        require(identity in expected and identity not in result, 'Missing, unexpected or duplicate identity')
        result[identity] = row
    require(set(result) == expected, 'Incomplete identity set')
    return result


def check(java, native, profile):
    check_origins(java['origins'])
    statuses = index(java['statuses'], ['code'], {(code,) for code in range(21)})
    for (code,), row in statuses.items():
        require(row.get('name') == NAMES[code], 'Java status name differs')
        for key, values in GROUPS.items():
            require(type(row.get(key)) is bool and row[key] == (code in values), 'Status classification differs: ' + key)
    transitions = index(java['transitions'], ['before', 'after'], {(x, y) for x in range(21) for y in range(21)})
    commit = GROUPS['commit_global']
    rollback = GROUPS['rollback_global'] | GROUPS['timeout_global']
    rejected = 0
    for (before, after), row in transitions.items():
        allowed = not ((before in commit and after in rollback) or (before in rollback and after in commit))
        require(type(row.get('allowed')) is bool and row['allowed'] == allowed, 'Validator result differs')
        rejected += not allowed
    unknown_java = index(java['unknown'], ['code'], {(code,) for code in range(21, 256)})
    for row in unknown_java.values():
        require(row.get('int_rejected') is True and row.get('byte_rejected') is True, 'Java accepted unknown status')
    identities = {(kind, status, result) for kind in TYPES for status in range(21) for result in [0, 1]}
    java_rows = index(java['responses'], ['type', 'status', 'result'], identities)
    for (kind, status, result), row in java_rows.items():
        message = '状态诊断:IO' if result == 0 else ''
        error = 3 if result == 0 else 0
        # 独立按已确认的 V1 字段格式构造预期，不把 Java 输出复制为正确答案。
        wire = struct.pack('>HB', kind, result)
        if result == 0:
            text = message.encode('utf-8')
            wire += struct.pack('>h', len(text)) + text
        wire += bytes([error, status])
        require(type(row.get('error')) is int and row['error'] == error and row.get('msg') == message, 'Java result fields differ')
        require(row.get('hex') == wire.hex(), 'Java bytes differ from independent wire contract')
    unknown_ids = {(kind, code) for kind in TYPES for code in range(21, 256)}
    for label, data in [('native', native), ('profile', profile)]:
        require(data.get('named_statuses') == list(range(16)) and all(type(x) is int for x in data['named_statuses']), 'Go named enum set differs')
        rows = index(data['responses'], ['type', 'status', 'result'], identities)
        for identity, row in rows.items():
            expected = java_rows[identity]
            require(type(row.get('error')) is int and row['error'] == expected['error'] and row.get('msg') == expected['msg'], 'Go result fields differ')
            require(row.get('hex') == expected['hex'] and row.get('encoded_hex') == expected['hex'], 'Go field/byte result differs')
        unknown = index(data['unknown'], ['type', 'code'], unknown_ids)
        for row in unknown.values():
            require(row.get('rejected') is (label == 'profile') and row.get('preserved') is (label == 'native'), 'Unknown status handling differs')
    return dict(statuses=21, predicate_observations=189, validator_pairs=441,
                validator_rejected_pairs=rejected, response_cases=168,
                java_unknown_statuses_rejected=235, native_unknown_payloads_preserved=940,
                profile_unknown_payloads_rejected=940, unnamed_go_statuses=list(range(16, 21)),
                business_semantics_complete=False, go_v1_state_001='OPEN')
