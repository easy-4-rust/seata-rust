"""Verify TableMeta's original76 cases and bounded dependency semantics; keep object closure open."""
import argparse
import copy
import datetime
import json
import os
import re
import shutil
import subprocess
import sys
from pathlib import Path
from freeze import ROOT,DOC
from qualify import read,sha
from verify import need,parse_values,compare
from verify_index import table
from verify_locale import replace_body

PARENT=ROOT/'docs/phase-2/evidence/sqlparser-locale-gate/run_20261118_052535_fb6c7ec8'
PARENT_SHA='c411a512731f65f154c5767bbb3b015af1ee14156537497eed4c4ab232cb34e9'


def baselines(oracle):
    source_keys={row['id'] for row in read(DOC/'inventory.json')['cases']}
    combined=b''.join((oracle/name).read_bytes() for name in ['foundation-java.tsv','column-java.tsv','index-java.tsv','table-java.tsv'])
    source={}
    for line in combined.decode('utf-8').splitlines():
        key,value=line.split('\t');need(key not in source,'Duplicate source case')
        source[key]=bytes.fromhex(value).decode('utf-8').split('\u001f')
    need(len(source_keys)==76 and set(source)==source_keys,'Original76 scope differs')
    index_all=table(oracle/'index-edges-java.tsv',148)
    identities={k:v for k,v in index_all.items() if k.startswith('enum-identity/')}
    index={k:v for k,v in index_all.items() if k not in identities}
    table_all=table(oracle/'table-edges-java.tsv',16)
    table_identities={k:v for k,v in table_all.items() if k.startswith('enum-identity/')}
    edges={k:(v[2:] if k.startswith('hierarchy/') else v) for k,v in table_all.items() if k not in table_identities}
    hashes={k:v[1:] for k,v in table(oracle/'primary-hash-java.tsv',283).items()}
    need(len(identities)==len(table_identities)==4 and len(edges)==12,'Input/result roles differ')
    sets_parent=ROOT/read(PARENT/'result.json')['parent']
    result={'source':source,'column':table(oracle/'column-edges-java.tsv',21),'index':index,
        'table-set':table(sets_parent/'verification/1.99-table-set.tsv',23370),
        'locale-casing':table(PARENT/'verification/1.99-locale-casing.tsv',12384),
        'locale-boundary':table(PARENT/'verification/1.99-locale-boundary.tsv',12384),
        'table-hash':hashes,'table-edge':edges}
    return result,identities,table_identities,combined


def run(oracle):
    need(sha(PARENT/'catalog.json')==PARENT_SHA,'Parent catalog drift')
    for row in read(PARENT/'catalog.json')['members']:need(sha(PARENT/row['path'])==row['sha256'],'Parent member drift')
    receipt=read(oracle/'oracle-result.json');need(receipt['parent_catalog_sha256']==PARENT_SHA,'Wrong parent')
    need(len(receipt['original_runtime_inputs'])==219 and len(receipt['jdk_runtime_inputs'])==5,'Runtime closure differs')
    for name,digest in (receipt['original_runtime_inputs']|receipt['jdk_runtime_inputs']).items():need(sha(Path(name))==digest,'Runtime drift')
    for row in receipt['producer_sources']+receipt['producer_classes']:need(sha(Path(row['path']))==row['sha256'],'Producer drift')
    sys.path.insert(0,str(ROOT/'tools/formal-source-junit'))
    from validate import load,validate_events,controls
    events=load(oracle/'junit-events.jsonl');original=validate_events(events,'seata-sqlparser-core');negative=controls(events,'seata-sqlparser-core')
    need(original['passed']==76 and negative['count']==13 and negative['all_rejected'],'Original JUnit closure differs')
    need(sha(ROOT/'Cargo.lock')==sha(PARENT/'inputs/Cargo.lock'),'Dependency lock changed')
    expected,index_identity,table_identity,combined=baselines(oracle)
    out=oracle/('verification_'+datetime.datetime.now().strftime('%Y%m%d_%H%M%S_%f'));out.mkdir()
    (out/'source-java.tsv').write_bytes(combined)
    inputs=[]
    for directory in ['crates','tools/formal-sqlparser-core','docs/phase-2/sqlparser_core']:
        inputs.extend(dict(path=p.relative_to(ROOT).as_posix(),sha256=sha(p)) for p in sorted((ROOT/directory).rglob('*')) if p.is_file())
    inputs.extend(dict(path=n,sha256=sha(ROOT/n)) for n in ['Cargo.toml','Cargo.lock'])
    variables={'SEATA_CORE_OBSERVATIONS':'source','SEATA_COLUMN_EDGE_OBSERVATIONS':'column',
        'SEATA_INDEX_EDGE_OBSERVATIONS':'index','SEATA_INDEX_IDENTITY_INPUTS':'identity',
        'SEATA_TABLE_SET_OBSERVATIONS':'table-set','SEATA_LOCALE_CASING_OBSERVATIONS':'locale-casing',
        'SEATA_LOCALE_BOUNDARY_OBSERVATIONS':'locale-boundary','SEATA_TABLE_HASH_OBSERVATIONS':'table-hash',
        'SEATA_TABLE_EDGE_OBSERVATIONS':'table-edge','SEATA_TABLE_IDENTITY_INPUTS':'table-identity'}
    fixture_vars={'SEATA_INDEX_ORACLE_FIXTURE':'index-edges-java.tsv','SEATA_TABLE_HASH_ORACLE':'primary-hash-java.tsv','SEATA_TABLE_EDGE_ORACLE':'table-edges-java.tsv'}
    base=os.environ|dict(CARGO_INCREMENTAL='0',CARGO_HOME='D:/codex-builds/seata-rust-cargo-home',TEMP='D:/Temp',TMP='D:/Temp')
    for n in variables|fixture_vars:base.pop(n,None)
    commands=[]
    def invoke(argv,name,env,cwd=ROOT,failure=False):
        with(out/name).open('wb') as log:
            code=subprocess.run(list(map(str,argv)),cwd=cwd,env=env,stdout=log,stderr=subprocess.STDOUT,timeout=600,creationflags=subprocess.CREATE_NO_WINDOW).returncode
        commands.append(dict(argv=list(map(str,argv)),cwd=str(cwd),exit_code=code,log=name,
            env={k:env[k] for k in list(variables|fixture_vars)+['CARGO_HOME','CARGO_TARGET_DIR','RUSTC','RUSTUP_TOOLCHAIN'] if k in env}))
        (out/'commands.json').write_text(json.dumps(commands,indent=2)+'\n','utf-8')
        need(code==(101 if failure else 0),'Unexpected exit: '+name)
        return(out/name).read_text('utf-8')
    results={}
    for version in ['1.99','1.85']:
        env=base|dict(CARGO_TARGET_DIR='D:/cargo-target/seata-rust-production'+('-msrv' if version=='1.85' else ''));cargo='cargo'
        if version=='1.85':
            binary=Path('C:/Users/hnxyh/.rustup/toolchains/1.85.0-x86_64-pc-windows-msvc/bin');cargo=binary/'cargo.exe'
            env.update(RUSTC=str(binary/'rustc.exe'),RUSTDOC=str(binary/'rustdoc.exe'),RUSTUP_TOOLCHAIN='1.85.0-x86_64-pc-windows-msvc',PATH=str(binary)+os.pathsep+env['PATH'])
        need('cargo '+version+'.' in invoke([cargo,'--version'],version+'-cargo.log',env),'Wrong toolchain')
        invoke([env.get('RUSTC','rustc'),'--version'],version+'-rustc.log',env)
        if version=='1.99':invoke([cargo,'fmt','--all','--','--check'],version+'-fmt.log',env)
        for name,features in [('default',[]),('all',['--all-features'])]:
            invoke([cargo,'check','--offline','--locked','--workspace','--all-targets']+features,version+'-check-'+name+'.log',env)
        invoke([cargo,'clippy','--offline','--locked','--workspace','--all-features','--all-targets','--','-D','warnings'],version+'-clippy.log',env)
        for variable,label in variables.items():env[variable]=str(out/(version+'-'+label+'.tsv'))
        for variable,name in fixture_vars.items():env[variable]=str(oracle/name)
        log=invoke([cargo,'test','--offline','--locked','--workspace','--all-features','--all-targets'],version+'-tests.log',env)
        totals=[tuple(map(int,r)) for r in re.findall(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored',log)]
        need(len(totals)==95 and sum(r[0] for r in totals)==471 and sum(r[1] for r in totals)==0 and sum(r[2] for r in totals)==143,'Full regression floor differs')
        for n in variables|fixture_vars:env.pop(n,None)
        log=invoke([cargo,'test','--offline','--locked','--workspace','--all-features','--doc'],version+'-doc.log',env)
        need(sum(map(int,re.findall(r'test result: ok\. (\d+) passed;',log)))==4,'Doc floor differs')
        for label,baseline in expected.items():
            actual=parse_values(out/(version+'-'+label+'.tsv'),baseline) if label=='source' else table(out/(version+'-'+label+'.tsv'),len(baseline))
            (out/(version+'-'+label+'-diff.json')).write_text(json.dumps(compare(baseline,actual),indent=2)+'\n','utf-8')
        need(table(out/(version+'-identity.tsv'),4)==index_identity,'Index JVM context input differs')
        need(table(out/(version+'-table-identity.tsv'),4)==table_identity,'Table JVM context input differs')
        log=invoke([cargo,'test','--offline','--locked','-p','seata-test','--all-targets'],version+'-acceptance.log',env)
        scoped=[tuple(map(int,r)) for r in re.findall(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored',log)]
        need(sum(r[0] for r in scoped)==131 and sum(r[1]+r[2] for r in scoped)==0,'Explicit acceptance package scope differs')
        results[version]=dict(ordinary=471,harness=95,ignored=143,doc=4,acceptance_package_passed=131,
            source_MATCH=76,primary_hash_order_records_MATCH=283,table_edge_records_MATCH=12,
            inherited_computed_observations_MATCH=23535,locale_context_records_MATCH=12384,locale_fields_per_record=3)
        print(json.dumps(dict(version=version,**results[version])),flush=True)
    work=Path(receipt['work'])/'mutants'/out.name
    need(work.resolve().is_relative_to(Path('D:/codex-builds').resolve()),'Mutation work escaped D')
    variants=[
        ('omit-map-hash-offset','util/lower_case_link_hash_map.rs','Ok(hash.wrapping_add(31))','Ok(hash)','table_java_diff','defaults_and_original_name_case_flag_equals_hash_quirks_match'),
        ('require-any-primary-column','struct/table_meta.rs','pk.iter().all(|name| cols.contains(name))','pk.iter().any(|name| cols.contains(name))','table_map_obligations','contains_pk_requires_every_key_and_preserves_nullable_collection_elements'),
        ('exact-exception-class-only','java_object_error.rs',None,None,'table_exception_hierarchy','map_equals_catches_both_exception_subclasses_after_direct_callback_retains_identity'),
        ('final-cursor-version-check','metadata_entry_cursor.rs',None,None,'table_map_obligations','map_equality_sees_future_replacement_and_keeps_last_callback_structure_shortcut'),
        ('ignore-case-flag-in-table-hash','struct/table_meta.rs','if self.is_case_sensitive()? {','if false {','table_java_diff','defaults_and_original_name_case_flag_equals_hash_quirks_match'),
        ('ignore-locale-lowercase','metadata_locale.rs',None,None,'table_java_diff','actual_language_specific_lowercase_map_keys_values_and_insertion_order_match')]
    mutants=[]
    template=Path(read(PARENT/'result.json')['mutants'][0]['path']).parents[3]/'Cargo.toml'
    for name,relative,old,new,test_file,test_name in variants:
        directory=work/name;directory.mkdir(parents=True)
        for crate in ['seata_sqlparser','seata_test']:shutil.copytree(ROOT/'crates'/crate,directory/'crates'/crate)
        (directory/'Cargo.toml').write_bytes(template.read_bytes())
        path=directory/'crates/seata_sqlparser/src'/relative;text=path.read_text('utf-8');before=sha(path)
        if name=='exact-exception-class-only':text=replace_body(text,'pub fn is_instance_of(','self.class_name == class_name')
        elif name=='ignore-locale-lowercase':text=replace_body(text,'pub fn lower(','Ok(value.clone())')
        elif name=='final-cursor-version-check':
            old='let Some(index) = self.next else {';new='let Some(index) = self.next else {\n            self.map.cursor_next(self.version, 0)?;'
            need(text.count(old)==1,'Cursor mutation anchor differs');text=text.replace(old,new)
        else:need(text.count(old)==1,'Mutation anchor differs '+name);text=text.replace(old,new)
        path.write_text(text,'utf-8')
        env=base|dict(CARGO_TARGET_DIR='D:/cargo-target/seata-table-mutants')
        log=invoke(['cargo','test','--offline','-p','seata-test','--test',test_file,test_name,'--','--exact'],'mutant-'+name+'.log',env,directory,True)
        need('test result: FAILED. 0 passed; 1 failed;' in log and 'error[E' not in log,'Mutation did not fail target behavior')
        mutants.append(dict(name=name,path=str(path),before_sha256=before,mutant_sha256=sha(path),result='RUNTIME_REJECTED'))
    controls_result=[]
    for label in ['source','table-hash','table-edge']:
        baseline=expected[label]
        for mode in ['missing','foreign','mismatch']:
            changed=copy.deepcopy(baseline)
            if mode=='missing':changed.pop(next(iter(changed)))
            if mode=='foreign':changed['foreign']=['true']
            if mode=='mismatch':changed[next(iter(changed))]=['wrong']
            try:compare(baseline,changed)
            except(ValueError,KeyError):controls_result.append(label+'-'+mode)
            else:raise ValueError('Accepted counterexample '+label+'-'+mode)
    for row in inputs:need(sha(ROOT/row['path'])==row['sha256'],'Live input drift')
    (out/'inputs.json').write_text(json.dumps(inputs,indent=2)+'\n','utf-8')
    result=dict(status='ORIGINAL76_CASE_FLOOR_VERIFIED_TABLE_META_PARTIAL',oracle=str(oracle),verification=str(out),Rust=results,
        source_object_states=dict(IMPLEMENTED=13,PARTIAL=1,MISSING=17),original_case_states=dict(MATCH=76,MISSING=0),
        original_JUnit_passed=76,original_JUnit_counterexamples_rejected=13,mutants=mutants,comparison_controls=controls_result,
        identity_contexts_input_not_result=['IndexMeta JVM:4','TableMeta JVM:4'],throwable_class_and_parent_descriptors_role='INPUT_NOT_RESULT',
        inherited_surviving_variants=read(PARENT/'result.json')['surviving_variants'],variant_equivalence_proven=False,
        registry_dependency_closure_changed=False,full_core_complete=False,WP05_complete=False,WP06_complete=False,
        full_V1_complete=False,TC_safety_gate_passed=False,external_TC_risk_accepted=False,shared_lock_issue='deferred_by_user',
        limitations=['Explicit language injection; OS/process default Locale bootstrap pending','getEscapePkNameList/ColumnUtils/EscapeHandler/SPI absent',
            'SqlServer subtype equals bridge absent','Full mutable Map entry/key/value views and default methods absent',
            '143 environment tests ignored; no new database qualification','Inherited unused Locale optimization survived; equivalence unproven'])
    (out/'result.json').write_text(json.dumps(result,indent=2)+'\n','utf-8')
    print(json.dumps(dict(verification=str(out),source_MATCH=76,TableMeta='PARTIAL',rejected_mutants=len(mutants))),flush=True)


if __name__=='__main__':
    parser=argparse.ArgumentParser();parser.add_argument('--oracle',type=Path,required=True);run(parser.parse_args().oracle.resolve())
