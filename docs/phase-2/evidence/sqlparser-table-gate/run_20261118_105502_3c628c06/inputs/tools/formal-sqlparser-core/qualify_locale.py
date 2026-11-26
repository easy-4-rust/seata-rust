"""Freeze Locale evidence, separating rejected defects, unclassified variants, inputs and source cases."""
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
from verify_locale import PARENT,expected


def run(verification):
    result=read(verification/'result.json');oracle=Path(result['oracle']);receipt=read(oracle/'oracle-result.json')
    need(sha(PARENT/'catalog.json')=='9df62e28460c66b2aae94071d417f2f10ce3c2a46eedf02af237b3daeb6845bb','Wrong parent')
    for row in read(PARENT/'catalog.json')['members']:need(sha(PARENT/row['path'])==row['sha256'],'Parent member drift')
    for name,digest in (receipt['original_runtime_inputs']|receipt['jdk_runtime_inputs']).items():need(sha(Path(name))==digest,'Runtime input drift')
    need(len(receipt['original_runtime_inputs'])==219 and len(receipt['jdk_runtime_inputs'])==5,'Runtime scope differs')
    for row in receipt['producer_sources']+receipt['producer_classes']:need(sha(Path(row['path']))==row['sha256'],'Producer drift')
    inventory=read(DOC/'inventory.json')
    for row in inventory['objects']:need(sha(SOURCE/row['java'])==row['sha256'],'Java source drift')
    for row in inventory['cases']:need(sha(SOURCE/row['source'])==row['source_sha256']==sha(ROOT/row['immutable_copy']),'Original test drift')
    need(sha(ROOT/'Cargo.lock')==sha(PARENT/'inputs/Cargo.lock'),'Dependency lock changed')
    unchanged=[]
    for p in (PARENT/'inputs/crates').rglob('*'):
        relative=p.relative_to(PARENT/'inputs')
        if p.is_file() and relative.parts[1] not in {'seata_sqlparser','seata_test'}:
            need(sha(ROOT/relative)==sha(p),'Pre-existing component changed');unchanged.append(relative.as_posix())
    locale,_=expected(oracle);casing={k:v[:2] for k,v in locale.items()};boundaries={k:v[2:] for k,v in locale.items()}
    index_gate=ROOT/read(PARENT/'result.json')['parent'];source_keys={row['id'] for row in read(PARENT/'verification/1.99-source-diff.json')}
    java=parse_values(index_gate/'verification/source-java.tsv',source_keys)
    prior_sets=table(PARENT/'verification/1.99-table-set.tsv',23370)
    column=table(index_gate/'oracle/column-edges-java.tsv',21);all_index=table(index_gate/'oracle/index-edges-java.tsv',148)
    identity={k:v for k,v in all_index.items() if k.startswith('enum-identity/')};index={k:v for k,v in all_index.items() if k not in identity}
    for version in ['1.99','1.85']:
        for label,baseline,observed in [('source',java,parse_values(verification/(version+'-source.tsv'),source_keys)),
            ('column',column,table(verification/(version+'-column.tsv'),21)),('index',index,table(verification/(version+'-index.tsv'),144)),
            ('table-set',prior_sets,table(verification/(version+'-table-set.tsv'),23370)),
            ('locale-casing',casing,table(verification/(version+'-locale-casing.tsv'),12384)),
            ('locale-boundary',boundaries,table(verification/(version+'-locale-boundary.tsv'),12384))]:
            need(compare(baseline,observed)==read(verification/(version+'-'+label+'-diff.json')),'Differential derivative differs')
        need(table(verification/(version+'-identity.tsv'),4)==identity,'Prior identity input differs')
        rows=[tuple(map(int,row)) for row in re.findall(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored',(verification/(version+'-tests.log')).read_text('utf-8'))]
        need(len(rows)==91 and sum(r[0] for r in rows)==440 and sum(r[1] for r in rows)==0 and sum(r[2] for r in rows)==143,'Actual regression scope differs')
        need(sum(map(int,re.findall(r'test result: ok\. (\d+) passed;',(verification/(version+'-doc.log')).read_text('utf-8'))))==4,'Doc scope differs')
    commands=read(verification/'commands.json')
    need(len(commands)==24 and all(row['exit_code']==(101 if row['log'].startswith('mutant-') else 0) for row in commands),'Actual command closure differs')
    need(len(result['mutants'])==8 and len(result['surviving_variants'])==1,'Mutation/variant classification differs')
    for row in result['mutants']:
        text=(verification/('mutant-'+row['name']+'.log')).read_text('utf-8')
        need('test result: FAILED. 0 passed; 1 failed;' in text and 'error[E' not in text and sha(Path(row['path']))==row['mutant_sha256'],'Mutation must fail behavior')
    variant=result['surviving_variants'][0]
    need(variant['result']=='SURVIVED_NO_COUNTEREXAMPLE_FOUND' and variant['equivalence_proven'] is False,'Unclassified variant promoted into proof')
    need('test result: ok. 1 passed; 0 failed;' in (verification/('variant-'+variant['name']+'.log')).read_text('utf-8') and sha(Path(variant['path']))==variant['mutant_sha256'],'Variant result drift')
    search=oracle/'order-diagnostics/run_20261118_052227_3bf61a39';search_result=read(search/'result.json')
    need(search_result['actual_JDK_searches']==353922 and search_result['equivalence_proven'] is False,'Bounded search promoted into equivalence')
    need((search/'search.tsv').read_text('utf-8')=='checked\t353922\nNO_COUNTEREXAMPLE_IN_DECLARED_SEARCH\ncursor-previous\t2\t55296\t1\n','Search or private cursor result differs')
    need(sha(search/'LocaleBoundaryOrderProbe.java')==search_result['source_sha256']==sha(ROOT/'tools/formal-sqlparser-core/LocaleBoundaryOrderProbe.java'),'Actual search producer drift')
    for row in search_result['producer_classes']:need(sha(Path(row['path']))==row['sha256'],'Search class drift')
    inputs=read(verification/'inputs.json')
    for row in inputs:need(sha(ROOT/row['path'])==row['sha256'],'Verified input drift')
    stamp=datetime.datetime.now().strftime('%Y%m%d_%H%M%S')+'_'+uuid.uuid4().hex[:8]
    out=ROOT/'docs/phase-2/evidence/sqlparser-locale-gate'/('run_'+stamp);out.mkdir(parents=True)
    for row in inputs:
        target=out/'inputs'/row['path'];target.parent.mkdir(parents=True,exist_ok=True);target.write_bytes((ROOT/row['path']).read_bytes());need(sha(target)==row['sha256'],'Snapshot drift')
    shutil.copytree(verification,out/'verification')
    (out/'oracle').mkdir()
    for p in oracle.iterdir():
        if p.is_file():shutil.copy2(p,out/'oracle'/p.name)
    shutil.copytree(oracle/'profiles',out/'oracle/profiles');shutil.copytree(search,out/'oracle/order-diagnostic')
    (out/'producer-classes').mkdir();(out/'mutant-sources').mkdir()
    for row in receipt['producer_classes']+search_result['producer_classes']:shutil.copy2(row['path'],out/'producer-classes'/Path(row['path']).name)
    for row in result['mutants']+result['surviving_variants']:shutil.copy2(row['path'],out/'mutant-sources'/(row['name']+'.rs'))
    for source,name in [('D:/Temp/seata-locale-structural-red.log','structural-red.log'),
        ('D:/Temp/seata-locale-category-validation-red.log','category-runtime-red.log'),
        ('D:/Temp/seata-locale-random-boundary-behavior-red.log','random-boundary-behavior-red.log'),
        ('D:/Temp/seata-locale-layout-final.json','layout-audit.json')]:shutil.copy2(source,out/name)
    layout=read(out/'layout-audit.json');need(layout['errors']==layout['strict_migration_blockers']==18 and layout['warnings']==0,'Unexpected layout scope')
    rejected=[]
    def reject(name,action):
        try:action()
        except(ValueError,KeyError):rejected.append(name);return
        raise ValueError('Accepted counterexample '+name)
    for label,baseline in [('source',java),('locale-casing',casing),('locale-boundary',boundaries)]:
        for mode in ['missing','foreign','mismatch']:
            changed=copy.deepcopy(baseline)
            if mode=='missing':changed.pop(next(iter(changed)))
            if mode=='foreign':changed['foreign']=['true']
            if mode=='mismatch':changed[next(iter(changed))]=['wrong']
            reject(label+'-'+mode,lambda:compare(baseline,changed))
        path=out/('duplicate-'+label+'.tsv');origin=index_gate/'verification/source-java.tsv' if label=='source' else verification/('1.99-'+label+'.tsv')
        path.write_bytes(origin.read_bytes()*2)
        reject('duplicate-'+label,lambda:parse_values(path,source_keys) if label=='source' else table(path,12384))
    (out/'comparison-counterexamples.json').write_text(json.dumps(dict(count=12,all_rejected=True,cases=rejected),indent=2)+'\n','utf-8')
    (out/'qualify-executed.py').write_bytes(Path(__file__).read_bytes())
    published=result|dict(parent=PARENT.relative_to(ROOT).as_posix(),parent_catalog_sha256=sha(PARENT/'catalog.json'),
        input_files=len(inputs),unchanged_existing_component_files=len(unchanged),comparison_counterexamples=12,
        original_test_sources=12,source_fixture_assets=0,original_JUnit_76_inherited_not_rerun=True,original_source_changes=False,
        bounded_actual_JDK_searches=353922,bounded_search_is_not_equivalence_proof=True,private_cursor_unit_is_not_source_case=True,
        strict_module_layout_blockers=18,
        historical_attempts=[dict(path=p.name,qualified=False) for p in oracle.glob('verification_*') if p!=verification],
        limitations=['One unused precomputed-boundary variant survives the declared corpus; equivalence unproven, not counted as a rejected defect',
            'Production preserves original per-Sigma query order; default system Locale management and TableMeta integration remain pending',
            '143 environment-dependent tests remain ignored; no new database qualification'])
    (out/'result.json').write_text(json.dumps(published,indent=2)+'\n','utf-8')
    members=[dict(path=p.relative_to(out).as_posix(),sha256=sha(p)) for p in sorted(out.rglob('*')) if p.is_file()]
    (out/'catalog.json').write_text(json.dumps(dict(members=members),indent=2)+'\n','utf-8')
    print(json.dumps(dict(root=str(out),catalog_sha256=sha(out/'catalog.json'),members=len(members),inputs=len(inputs),rejected_mutants=8,unclassified_variants=1)),flush=True)


if __name__=='__main__':
    parser=argparse.ArgumentParser();parser.add_argument('--verification',type=Path,required=True);run(parser.parse_args().verification.resolve())
