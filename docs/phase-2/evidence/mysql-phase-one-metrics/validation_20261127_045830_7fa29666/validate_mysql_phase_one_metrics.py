"""Replay bounded stage observations against sealed native and original TC runs."""
import copy
import datetime
import json
from pathlib import Path
import re
import shutil
import uuid

from inventory_rebuilt import ROOT, need, save, sha
from qualify_rebuilt import verify_catalog

BASE = ROOT / 'docs/phase-2/evidence/mysql-phase-one-metrics'
NATIVE = BASE / 'run_20261127_043647_ab444f84'
JAVA = BASE / 'java_20261127_044228_b6137836'
NATIVE_SHA = '922cc4f70a5b42986dcda93a1c11cfe563492cac10a31354b11b29e2f4ed1766'
SHAPE = {
    'prepare': 'prepared_unregistered prepared_registered registration_unknown prepare_failed cancelled',
    'check_commit': 'checked_without_witness checked_with_witness check_failed cancelled',
    'commit_no_change': 'no_change_committed no_change_commit_unknown no_change_not_eligible cancelled',
    'commit_changed': 'changed_committed changed_commit_unknown changed_not_eligible arming_failed arming_uncertain cancelled',
    'commit_and_report': 'report_observed report_local_failed report_local_uncertain report_claim_failed report_unknown report_failed cancelled',
}


def blank():
    return {op: dict(active=0, outcomes=dict.fromkeys(labels.split(), 0)) for op, labels in SHAPE.items()}


def profile(*labels):
    value = blank()
    for label in labels:
        op, outcome = label.split('.')
        value[op]['outcomes'][outcome] += 1
    return value


PREP = 'prepare.prepared_registered'
CHECK = 'check_commit.checked_with_witness'
DONE = 'commit_changed.changed_committed'
NO_WITNESS = 'check_commit.checked_without_witness'
EMPTY = 'prepare.prepared_unregistered'
NATIVE_PROFILES = [
    [profile('prepare.prepare_failed'), profile('prepare.registration_unknown')],
    [profile('prepare.cancelled')],
    [profile('prepare.registration_unknown'), profile('prepare.cancelled')],
    [profile('prepare.prepare_failed')], [blank()],
    [profile(PREP, CHECK, DONE)],
    [profile(PREP, CHECK, 'commit_changed.arming_failed')],
    [profile(PREP, NO_WITNESS, 'commit_changed.changed_not_eligible')],
    [profile(PREP, CHECK, 'commit_changed.changed_commit_unknown', 'commit_and_report.report_local_uncertain')],
    [profile(PREP, CHECK, 'commit_changed.changed_commit_unknown')],
    [profile(PREP, CHECK, DONE, 'commit_and_report.report_observed')],
    [profile(PREP, CHECK, DONE, 'commit_and_report.report_failed')],
    [profile(EMPTY, NO_WITNESS, 'commit_changed.changed_not_eligible', 'commit_and_report.report_local_failed')],
]
JAVA_PROFILES = {
    'no_change_commit': [profile(EMPTY, NO_WITNESS, 'commit_no_change.no_change_committed')],
    'no_change_ack_loss': [profile(EMPTY, NO_WITNESS, 'commit_no_change.no_change_commit_unknown')],
    'commit': [profile(PREP, CHECK, DONE, 'commit_and_report.report_observed')],
    'commit_report_loss': [profile(PREP, CHECK, DONE, 'commit_and_report.report_unknown')],
    'commit_report_claim_loss': [profile(PREP, CHECK, DONE)],
    'armed_child_arm_ack_loss': [profile(PREP, CHECK, 'commit_changed.arming_uncertain'), blank()],
}


def shape(value):
    need(set(value) == set(SHAPE), 'Unexpected stage or identity label')
    for op, labels in SHAPE.items():
        need(set(value[op]) == {'active', 'outcomes'}, 'Stage fields differ')
        need(set(value[op]['outcomes']) == set(labels.split()), 'Outcome labels differ')
        numbers = [value[op]['active'], *value[op]['outcomes'].values()]
        need(all(type(n) is int and 0 <= n <= 2**64 - 1 for n in numbers), 'Invalid counter type or range')


def profiles(actual, wanted):
    for value in actual:
        shape(value)
    key = lambda value: json.dumps(value, sort_keys=True)
    need(sorted(map(key, actual)) == sorted(map(key, wanted)), 'Stage outcomes differ from executed scenario')


def http(text, wanted):
    lines = {}
    for line in text.splitlines():
        match = re.fullmatch(r'(seata_mysql_phase_one_[^ ]+) ([0-9]+)', line)
        need(match is not None and match[1] not in lines, 'Malformed or duplicate series')
        lines[match[1]] = int(match[2])
    expected = {}
    for op, values in wanted.items():
        expected[f'seata_mysql_phase_one_active{{operation="{op}"}}'] = values['active']
        for outcome, count in values['outcomes'].items():
            expected[f'seata_mysql_phase_one_total{{operation="{op}",outcome="{outcome}"}}'] = count
    need(lines == expected and len(lines) == 31, 'HTTP fixed series differ')


def controls():
    rejected = []
    for label in ['prepared_as_committed', 'arming_as_business_unknown', 'report_loss_as_local_unknown',
                  'active_leak', 'boolean_counter', 'identity_label', 'missing_outcome', 'duplicate_observation']:
        wanted = JAVA_PROFILES['armed_child_arm_ack_loss'] if label == 'arming_as_business_unknown' else JAVA_PROFILES['commit_report_loss']
        actual = copy.deepcopy(wanted)
        value = actual[0]
        if label == 'prepared_as_committed':
            value['prepare']['outcomes']['prepared_registered'] = 0
            value['commit_changed']['outcomes']['changed_committed'] += 1
        elif label == 'arming_as_business_unknown':
            value['commit_changed']['outcomes'].update(arming_uncertain=0, changed_commit_unknown=1)
        elif label == 'report_loss_as_local_unknown':
            value['commit_changed']['outcomes'].update(changed_committed=0, changed_commit_unknown=1)
        elif label == 'active_leak':
            value['prepare']['active'] = 1
        elif label == 'boolean_counter':
            value['prepare']['outcomes']['prepared_registered'] = True
        elif label == 'identity_label':
            value['xid'] = 'private'
        elif label == 'missing_outcome':
            value['prepare']['outcomes'].pop('cancelled')
        else:
            actual.append(copy.deepcopy(value))
        try:
            profiles(actual, wanted)
        except ValueError:
            rejected.append(label)
        else:
            raise ValueError('Negative control accepted: ' + label)
    return rejected


def run():
    verify_catalog(NATIVE, NATIVE_SHA)
    java_digest = sha(JAVA / 'catalog.json')
    verify_catalog(JAVA, java_digest)
    records = json.loads((JAVA / 'scenarios.json').read_bytes())
    need(len(records) == 12 and len({(r['Rust'], r['scenario']) for r in records}) == 12, 'TC scenario set incomplete')
    for version in ['1.99', '1.85']:
        obs = NATIVE / 'observations' / version
        for index, expected in enumerate(NATIVE_PROFILES):
            actual = [json.loads(p.read_bytes()) for p in (obs / ('case-' + str(index))).glob('phase-one-stage-*.json')]
            profiles(actual, expected)
        http((obs / 'http/phase-one-http-zero.txt').read_text(encoding='utf-8'), blank())
        final = profile('prepare.prepare_failed')
        http((obs / 'http/phase-one-http-after-close.txt').read_text(encoding='utf-8'), final)
    for row in records:
        folder = ROOT / row['evidence']
        need(sha(folder / 'verified.json') == row['verified_sha256'], 'Original TC qualification drift')
        need(json.loads((folder / 'process-cleanup.json').read_bytes())['stopped'], 'Original TC process leaked')
        actual = []
        for member in row['stage_files']:
            p = folder / member['path']
            need(sha(p) == member['sha256'], 'TC stage observation drift')
            actual.append(json.loads(p.read_bytes()))
        profiles(actual, JAVA_PROFILES[row['scenario']])
        verified = json.loads((folder / 'verified.json').read_bytes())
        if row['scenario'] == 'armed_child_arm_ack_loss':
            need(verified['business_commit_requests'] == 0 and verified['durable_arm_commits'] == 1 and verified['arm_ack_dropped'], 'Arming vs business boundary differs')
        elif row['scenario'] == 'commit_report_loss':
            need(verified['phase_one_done_reports'] == 1 and verified['phase_one_report_success_responses'] == 1 and verified['report_response_delivered'] == 0, 'Original report ACK loss differs')
    need(json.loads((JAVA / 'cleanup.json').read_bytes())['owned_container_removed'], 'Java container leaked')
    need(json.loads((NATIVE / 'cleanup.json').read_bytes())['owned_container_removed'], 'Native container leaked')
    # 两处纯测试 lint 修订可逐字还原到实跑输入；生产代码不得漂移。
    lint_paths = {'crates/seata_observability/tests/mysql_recovery.rs', 'crates/seata_db_mysql/tests/support/first_phase_fixture.rs'}
    drift = []
    for row in json.loads((NATIVE / 'source-inputs.json').read_bytes()):
        p = ROOT / row['path']
        if sha(p) != row['sha256']:
            need(row['path'] in lint_paths, 'Consumed production source drift')
            drift.append(dict(path=row['path'], executed_sha256=row['sha256'], current_sha256=sha(p), reason='test-only lint attributes'))
    out = BASE / ('validation_' + datetime.datetime.now().strftime('%Y%m%d_%H%M%S') + '_' + uuid.uuid4().hex[:8])
    out.mkdir()
    save(out / 'result.json', dict(status='PHASE_ONE_STAGE_OBSERVATIONS_REPLAYED', native_catalog_sha256=NATIVE_SHA,
         java_catalog_sha256=java_digest, native_cases_per_version=14, original_TC_cases=12,
         fixed_http_series=31, negative_controls=controls(), test_only_lint_changes=drift,
         independent_human_or_other_agent_review='OPEN', WP07_complete=False))
    shutil.copyfile(Path(__file__), out / Path(__file__).name)
    save(out / 'catalog.json', [dict(path=p.name, sha256=sha(p)) for p in sorted(out.iterdir()) if p.is_file()])
    print(json.dumps(dict(validation=str(out), catalog_sha256=sha(out / 'catalog.json'))))


if __name__ == '__main__':
    run()
