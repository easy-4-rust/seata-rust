"""Seal current dependency delivery and the independent read-only review; full goal stays open."""
import json
import re
from freeze import ROOT, DOC
from qualify import read, sha
from verify import need
from publish_table_set import GATE, DIGEST


def save(path,value):path.write_text(json.dumps(value,ensure_ascii=False,indent=2)+'\n','utf-8')


def finalize():
    family=GATE.parent;catalog=read(GATE/'catalog.json')['members']
    need(sha(GATE/'catalog.json')==DIGEST,'Frozen catalog drift')
    for row in catalog:need(sha(GATE/row['path'])==row['sha256'],'Frozen member drift')
    for row in read(GATE/'verification/inputs.json'):
        if row['path'].startswith('crates/') or row['path'] in {'Cargo.toml','Cargo.lock'}:
            need(sha(ROOT/row['path'])==row['sha256'],'Verified production/test bytes drift')
    log=family/'explicit-acceptance-package-20261118.log'
    need(sum(map(int,re.findall(r'test result: ok\. (\d+) passed;',log.read_text('utf-8'))))==98,'Explicit package count differs')
    need(read(family/'current-module-final-audit-20261118.json')['status']=='FULL_MODULE_COMPLETION_CORRECTLY_BLOCKED','Full module no longer blocked')
    review=dict(reviewer='/root/sqlparser_values_review',mode='READ_ONLY',date='2026-11-18',confirmed_remaining_P1=0,confirmed_remaining_P2=0,
        confirmed_and_fixed_P2=['Nontransitive original JDK comparator makes sorted Vec iteration differ from TreeMap; actual RED and exact tree fix verified'],
        verified=['537 catalog members and468 inputs, parent catalog',
            'Actual both-toolchain437 ordinary/90 harness/143 ignored/4doc',
            '57 original source observations,165 inherited computed observations and23370 new computed observations MATCH',
            '2799 character map and4 enum identity records explicitly INPUT, not MATCH',
            '22 actual commands,7 runtime assertion failures and8 comparison negative controls independently checked',
            '219 original runtime and5 actual JDK files plus producer source/class binding',
            'Actual cycle behavior RED and original insertion/color/rotation/root search semantics',
            'Original JUnit76 explicitly inherited, not mislabeled as a fresh execution'],
        scope=dict(IMPLEMENTED=13,MISSING=18,original_cases_MATCH=57,original_cases_MISSING=19),
        full_core_complete=False,full_V1_complete=False)
    target=family/'readonly-review-20261118.json';need(not target.exists(),'Do not overwrite review');save(target,review)
    files=[ROOT/'README.md',ROOT/'Cargo.toml',ROOT/'Cargo.lock',ROOT/'docs/delivery-progress.md',
        ROOT/'docs/phase-2/wp05-sqlparser-table-prerequisites.json',family/'README.md',target,
        family/'current-module-final-audit-20261118.json',log]
    for directory in [DOC,ROOT/'tools/formal-sqlparser-core',ROOT/'crates/seata_sqlparser',ROOT/'crates/seata_test']:
        files.extend(path for path in directory.rglob('*') if path.is_file())
    publication=dict(status='TABLE_PRIMARY_SET_DEPENDENCY_PUBLISHED_FULL_GOAL_OPEN',gate=GATE.relative_to(ROOT).as_posix(),
        catalog_sha256=DIGEST,members=len(catalog),verification_inputs=468,
        object_states=dict(IMPLEMENTED=13,MISSING=18),original_case_states=dict(MATCH=57,MISSING=19),
        new_computed_observations_MATCH=23370,character_mapping_inputs_not_MATCH=2799,
        inherited_computed_observations_MATCH=165,inherited_enum_identity_inputs_not_MATCH=4,package_test_execution_passed=98,
        post_gate_change='Only current ledgers/docs and qualification/publication tools; production/test bytes remain the exact final verified inputs',
        full_core_complete=False,WP05_complete=False,WP06_complete=False,full_V1_complete=False,
        TC_safety_gate_passed=False,external_TC_risk_accepted=False,shared_lock_issue='deferred_by_user',
        files=[dict(path=path.relative_to(ROOT).as_posix(),sha256=sha(path)) for path in sorted(set(files))])
    target=family/'publication-20261118.json';need(not target.exists(),'Do not overwrite publication');save(target,publication)
    for row in publication['files']:need(sha(ROOT/row['path'])==row['sha256'],'Published current file drift')
    print(json.dumps(dict(publication=target.relative_to(ROOT).as_posix(),sha256=sha(target),files=len(publication['files']),
        source_MATCH=57,source_MISSING=19,new_computed_observations_MATCH=23370,full_core_complete=False)),flush=True)


if __name__=='__main__':finalize()
