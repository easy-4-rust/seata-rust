"""Independently recheck and freeze TableMeta case evidence without closing the partial object."""
import argparse
import copy
import datetime
import json
import re
import shutil
import uuid
from pathlib import Path
from freeze import ROOT,DOC,SOURCE
from qualify import read,sha
from verify import need,parse_values,compare
from verify_index import table
from verify_table import PARENT,PARENT_SHA,baselines


def run(verification):
    result=read(verification/'result.json');oracle=Path(result['oracle']);receipt=read(oracle/'oracle-result.json')
    need(sha(PARENT/'catalog.json')==PARENT_SHA,'Parent catalog drift')
    for row in read(PARENT/'catalog.json')['members']:need(sha(PARENT/row['path'])==row['sha256'],'Parent member drift')
    for name,digest in (receipt['original_runtime_inputs']|receipt['jdk_runtime_inputs']).items():need(sha(Path(name))==digest,'Runtime drift')
    for row in receipt['producer_sources']+receipt['producer_classes']:need(sha(Path(row['path']))==row['sha256'],'Producer drift')
    inventory=read(DOC/'inventory.json')
    for row in inventory['objects']:need(sha(SOURCE/row['java'])==row['sha256'],'Original object drift')
    for row in inventory['cases']:need(sha(SOURCE/row['source'])==row['source_sha256']==sha(ROOT/row['immutable_copy']),'Original test drift')
    need(sha(ROOT/'Cargo.lock')==sha(PARENT/'inputs/Cargo.lock'),'Dependency lock changed')
    unchanged=[]
    for p in(PARENT/'inputs/crates').rglob('*'):
        relative=p.relative_to(PARENT/'inputs')
        if p.is_file() and relative.parts[1] not in {'seata_sqlparser','seata_test'}:
            need(sha(ROOT/relative)==sha(p),'Other component drift');unchanged.append(relative.as_posix())
    expected,index_identity,table_identity,combined=baselines(oracle)
    need((verification/'source-java.tsv').read_bytes()==combined,'Combined original observations drift')
    for version in ['1.99','1.85']:
        for label,baseline in expected.items():
            actual=parse_values(verification/(version+'-'+label+'.tsv'),baseline) if label=='source' else table(verification/(version+'-'+label+'.tsv'),len(baseline))
            need(compare(baseline,actual)==read(verification/(version+'-'+label+'-diff.json')),'Derived differential drift')
        need(table(verification/(version+'-identity.tsv'),4)==index_identity,'Index JVM context drift')
        need(table(verification/(version+'-table-identity.tsv'),4)==table_identity,'Table JVM context drift')
        rows=[tuple(map(int,r)) for r in re.findall(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored',(verification/(version+'-tests.log')).read_text('utf-8'))]
        need(len(rows)==95 and sum(r[0] for r in rows)==471 and sum(r[1] for r in rows)==0 and sum(r[2] for r in rows)==143,'Actual full regression scope differs')
        scoped=[tuple(map(int,r)) for r in re.findall(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored',(verification/(version+'-acceptance.log')).read_text('utf-8'))]
        need(sum(r[0] for r in scoped)==131 and sum(r[1]+r[2] for r in scoped)==0,'Acceptance scope differs')
        need(sum(map(int,re.findall(r'test result: ok\. (\d+) passed;',(verification/(version+'-doc.log')).read_text('utf-8'))))==4,'Doc scope differs')
    commands=read(verification/'commands.json')
    need(len(commands)==23 and all(r['exit_code']==(101 if r['log'].startswith('mutant-') else 0) for r in commands),'Actual command closure differs')
    need(len(result['mutants'])==6,'Runtime mutant count differs')
    for row in result['mutants']:
        log=(verification/('mutant-'+row['name']+'.log')).read_text('utf-8')
        need('test result: FAILED. 0 passed; 1 failed;' in log and 'error[E' not in log and sha(Path(row['path']))==row['mutant_sha256'],'Not a behavior rejection')
    inputs=read(verification/'inputs.json')
    for row in inputs:need(sha(ROOT/row['path'])==row['sha256'],'Verified input drift')
    stamp=datetime.datetime.now().strftime('%Y%m%d_%H%M%S')+'_'+uuid.uuid4().hex[:8]
    out=ROOT/'docs/phase-2/evidence/sqlparser-table-gate'/('run_'+stamp);out.mkdir(parents=True)
    for row in inputs:
        target=out/'inputs'/row['path'];target.parent.mkdir(parents=True,exist_ok=True)
        shutil.copy2(ROOT/row['path'],target);need(sha(target)==row['sha256'],'Input snapshot drift')
    shutil.copytree(verification,out/'verification');(out/'oracle').mkdir()
    for p in oracle.iterdir():
        if p.is_file():shutil.copy2(p,out/'oracle'/p.name)
    (out/'producer-classes').mkdir();(out/'mutant-sources').mkdir()
    for row in receipt['producer_classes']:shutil.copy2(row['path'],out/'producer-classes'/Path(row['path']).name)
    for row in result['mutants']:shutil.copy2(row['path'],out/'mutant-sources'/(row['name']+'.rs'))
    for path,name in [('D:/Temp/seata-table-meta-structural-red.log','structural-red.log'),
        ('D:/Temp/seata-table-hierarchy-behavior-red.log','hierarchy-behavior-red.log'),
        ('D:/Temp/seata-table-layout-final.json','layout-audit.json')]:shutil.copy2(path,out/name)
    layout=read(out/'layout-audit.json');need(layout['errors']==layout['strict_migration_blockers']==17 and layout['warnings']==0,'Unexpected layout scope')
    rejected=[]
    def reject(name,action):
        try:action()
        except(ValueError,KeyError):rejected.append(name);return
        raise ValueError('Accepted counterexample '+name)
    for label in ['source','table-hash','table-edge']:
        baseline=expected[label]
        for mode in ['missing','foreign','mismatch']:
            changed=copy.deepcopy(baseline)
            if mode=='missing':changed.pop(next(iter(changed)))
            if mode=='foreign':changed['foreign']=['true']
            if mode=='mismatch':changed[next(iter(changed))]=['wrong']
            reject(label+'-'+mode,lambda:compare(baseline,changed))
        path=out/('duplicate-'+label+'.tsv');origin=verification/('1.99-'+label+'.tsv');path.write_bytes(origin.read_bytes()*2)
        reject(label+'-duplicate',lambda:parse_values(path,baseline) if label=='source' else table(path,len(baseline)))
    (out/'comparison-counterexamples.json').write_text(json.dumps(dict(count=12,all_rejected=True,cases=rejected),indent=2)+'\n','utf-8')
    (out/'qualify-executed.py').write_bytes(Path(__file__).read_bytes())
    published=result|dict(parent=PARENT.relative_to(ROOT).as_posix(),parent_catalog_sha256=PARENT_SHA,
        input_files=len(inputs),unchanged_existing_component_files=len(unchanged),comparison_counterexamples=12,
        original_JUnit_76_new_execution=True,original_source_changes=False,original_test_sources=12,source_fixture_assets=0,
        strict_module_layout_blockers=17,source_test_floor_is_not_full_object_coverage=True,
        historical_attempts=[dict(path=p.name,qualified=False) for p in oracle.glob('verification_*') if p!=verification])
    (out/'result.json').write_text(json.dumps(published,indent=2)+'\n','utf-8')
    members=[dict(path=p.relative_to(out).as_posix(),sha256=sha(p)) for p in sorted(out.rglob('*')) if p.is_file()]
    (out/'catalog.json').write_text(json.dumps(dict(members=members),indent=2)+'\n','utf-8')
    print(json.dumps(dict(root=str(out),catalog_sha256=sha(out/'catalog.json'),members=len(members),inputs=len(inputs),source_MATCH=76,TableMeta='PARTIAL')),flush=True)


if __name__=='__main__':
    parser=argparse.ArgumentParser();parser.add_argument('--verification',type=Path,required=True);run(parser.parse_args().verification.resolve())
