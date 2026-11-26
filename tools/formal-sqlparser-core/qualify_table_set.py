"""Freeze exact comparator/set dependency evidence; source module closure remains blocked."""
import argparse
import copy
import datetime
import json
import re
import shutil
import uuid
from pathlib import Path
from freeze import ROOT, DOC, SOURCE
from qualify import read, sha
from verify import need, parse_values, compare
from verify_index import table
from verify_table_set import PARENT, expected_observations


def run(verification):
    result=read(verification/'result.json');oracle=Path(result['oracle']);receipt=read(oracle/'oracle-result.json')
    need(sha(PARENT/'catalog.json')=='6c44f71b71cf392a52fe6e9e4b8675af11da1cba70b5c1bddc4f0a8d57b69774','Wrong parent')
    for row in read(PARENT/'catalog.json')['members']:need(sha(PARENT/row['path'])==row['sha256'],'Frozen parent drift')
    for name,digest in receipt['original_runtime_inputs'].items():need(sha(Path(name))==digest,'Original runtime drift')
    need(len(receipt['original_runtime_inputs'])==219 and len(receipt['jdk_runtime_inputs'])==5,'Runtime binding scope differs')
    for name,digest in receipt['jdk_runtime_inputs'].items():need(sha(Path(name))==digest,'JDK runtime drift')
    for row in receipt['producer_classes']:need(sha(Path(row['path']))==row['sha256'],'Actual producer class drift')
    need(sha(ROOT/'tools/formal-sqlparser-core/MetadataCaseOracle.java')==sha(oracle/'MetadataCaseOracle.java')==receipt['producer_source_sha256'],'Producer source drift')
    inventory=read(DOC/'inventory.json')
    for row in inventory['objects']:need(sha(SOURCE/row['java'])==row['sha256'],'Original Java source drift')
    for row in inventory['cases']:need(sha(SOURCE/row['source'])==row['source_sha256']==sha(ROOT/row['immutable_copy']),'Original test source drift')
    need(sha(ROOT/'Cargo.lock')==sha(PARENT/'inputs/Cargo.lock'),'Dependency lock changed')
    unchanged=[]
    for path in (PARENT/'inputs/crates').rglob('*'):
        relative=path.relative_to(PARENT/'inputs')
        if path.is_file() and relative.parts[1] not in {'seata_sqlparser','seata_test'}:
            need(sha(ROOT/relative)==sha(path),'Existing component changed');unchanged.append(relative.as_posix())
    need(sha(ROOT/'crates/seata_sqlparser/src/data/java17_character_case.bin')==sha(oracle/'java17-character-case-input.bin'),'Character input drift')
    expected=expected_observations(oracle)
    source_keys={row['id'] for row in read(PARENT/'verification/1.99-source-diff.json')}
    java=parse_values(PARENT/'verification/source-java.tsv',source_keys)
    column=table(PARENT/'oracle/column-edges-java.tsv',21)
    index_all=table(PARENT/'oracle/index-edges-java.tsv',148)
    identity={key:value for key,value in index_all.items() if key.startswith('enum-identity/')}
    index={key:value for key,value in index_all.items() if key not in identity}
    for version in ['1.99','1.85']:
        for label,baseline,observed in [('source',java,parse_values(verification/(version+'-source.tsv'),source_keys)),
            ('column',column,table(verification/(version+'-column.tsv'),21)),('index',index,table(verification/(version+'-index.tsv'),144)),
            ('table-set',expected,table(verification/(version+'-table-set.tsv'),23370))]:
            need(compare(baseline,observed)==read(verification/(version+'-'+label+'-diff.json')),'Derived differential differs')
        need(table(verification/(version+'-identity.tsv'),4)==identity,'Identity input binding differs')
        rows=[tuple(map(int,row)) for row in re.findall(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored',(verification/(version+'-tests.log')).read_text('utf-8'))]
        need(len(rows)==90 and sum(r[0] for r in rows)==437 and sum(r[1] for r in rows)==0 and sum(r[2] for r in rows)==143,'Regression floor differs')
        need(sum(map(int,re.findall(r'test result: ok\. (\d+) passed;',(verification/(version+'-doc.log')).read_text('utf-8'))))==4,'Doc floor differs')
    commands=read(verification/'commands.json')
    need(len(commands)==22 and all(row['exit_code']==(101 if row['log'].startswith('mutant-') else 0) for row in commands),'Actual command closure incomplete')
    need(len(result['mutants'])==7,'Mutant scope differs')
    for row in result['mutants']:
        need(sha(Path(row['path']))==row['mutant_sha256'],'Mutant source drift')
        text=(verification/('mutant-'+row['name']+'.log')).read_text('utf-8')
        need('test result: FAILED. 0 passed; 1 failed;' in text and 'error[E' not in text,'Nonbehavioral mutant failure')
    inputs=read(verification/'inputs.json')
    for row in inputs:need(sha(ROOT/row['path'])==row['sha256'],'Verified input drift')
    stamp=datetime.datetime.now().strftime('%Y%m%d_%H%M%S')+'_'+uuid.uuid4().hex[:8]
    out=ROOT/'docs/phase-2/evidence/sqlparser-table-prerequisites-gate'/('run_'+stamp);out.mkdir(parents=True)
    for row in inputs:
        target=out/'inputs'/row['path'];target.parent.mkdir(parents=True,exist_ok=True);target.write_bytes((ROOT/row['path']).read_bytes())
        need(sha(target)==row['sha256'],'Snapshot drift')
    for source,label in [(verification,'verification'),(oracle,'oracle')]:
        (out/label).mkdir()
        for path in source.iterdir():
            if path.is_file():shutil.copy2(path,out/label/path.name)
    for directory in ['producer-classes','mutant-sources']:(out/directory).mkdir()
    for row in receipt['producer_classes']:shutil.copy2(row['path'],out/'producer-classes'/Path(row['path']).name)
    for row in result['mutants']:shutil.copy2(row['path'],out/'mutant-sources'/(row['name']+'.rs'))
    for source,name in [('D:/Temp/seata-table-set-structural-red.log','structural-red.log'),
        ('D:/Temp/seata-table-set-cycle-behavior-red.log','cycle-behavior-red.log'),
        ('D:/Temp/seata-table-set-layout-final.json','layout-audit.json')]:shutil.copy2(source,out/name)
    layout=read(out/'layout-audit.json')
    need(layout['errors']==layout['strict_migration_blockers']==18 and layout['warnings']==0,'Unexpected layout blocker scope')
    text=(out/'cycle-behavior-red.log').read_text('utf-8')
    need('test result: FAILED. 0 passed; 1 failed;' in text and 'set/13' in text and 'error[E' not in text,'Original cycle was not behavior RED')
    rejected=[]
    def reject(name,action):
        try:action()
        except (ValueError,KeyError):rejected.append(name);return
        raise ValueError('Accepted counterexample '+name)
    for label,baseline in [('source',java),('table-set',expected)]:
        for mode in ['missing','foreign','mismatch']:
            changed=copy.deepcopy(baseline)
            if mode=='missing':changed.pop(next(iter(changed)))
            if mode=='foreign':changed['foreign']=['true']
            if mode=='mismatch':changed[next(iter(changed))]=['wrong']
            reject(label+'-'+mode,lambda:compare(baseline,changed))
    duplicate=out/'duplicate-source.tsv';duplicate.write_bytes((PARENT/'verification/source-java.tsv').read_bytes()*2)
    reject('duplicate-source',lambda:parse_values(duplicate,source_keys))
    duplicate=out/'duplicate-table-set.tsv';duplicate.write_bytes((verification/'1.99-table-set.tsv').read_bytes()*2)
    reject('duplicate-table-set',lambda:table(duplicate,23370))
    (out/'comparison-counterexamples.json').write_text(json.dumps(dict(count=8,all_rejected=True,cases=rejected),indent=2)+'\n','utf-8')
    (out/'qualify-executed.py').write_bytes(Path(__file__).read_bytes())
    published=result|dict(parent=PARENT.relative_to(ROOT).as_posix(),parent_catalog_sha256=sha(PARENT/'catalog.json'),
        input_files=len(inputs),unchanged_existing_component_files=len(unchanged),original_JUnit_76_inherited_not_rerun=True,
        original_test_sources=12,source_fixture_assets=0,strict_module_layout_blockers=18,
        additional_computed_observations_MATCH=23370,comparison_counterexamples=8,
        fixed_P2='Sorted Vec cannot preserve original TreeMap shape under nontransitive CASE_INSENSITIVE_ORDER; real RED and exact insertion/rotation fix',
        historical_attempts=[dict(path='run_20261118_042913_e5fa5e98',state='Initial observations lacked comparator-cycle cases; not qualified'),
            dict(path='run_20261118_043619_d31e028d/verification_20261118_043923_562964',state='Concurrent observer writes interleaved; rejected'),
            dict(path='run_20261118_043619_d31e028d/verification_20261118_044021_019061',state='MSRV clippy format_collect rejected observer encoding'),
            dict(path='run_20261118_044136_88a7c63f/verification_20261118_044147_055702',state='Successful precursor before final Chinese source comments; final input set is later explicit run')])
    (out/'result.json').write_text(json.dumps(published,indent=2)+'\n','utf-8')
    members=[dict(path=p.relative_to(out).as_posix(),sha256=sha(p)) for p in sorted(out.rglob('*')) if p.is_file()]
    (out/'catalog.json').write_text(json.dumps(dict(members=members),indent=2)+'\n','utf-8')
    print(json.dumps(dict(root=str(out),catalog_sha256=sha(out/'catalog.json'),members=len(members),inputs=len(inputs))),flush=True)


if __name__=='__main__':
    parser=argparse.ArgumentParser();parser.add_argument('--verification',type=Path,required=True)
    run(parser.parse_args().verification.resolve())
