"""Fresh full-workspace/MSRV gate:57 source cases,165 computed edges,4 explicit identity inputs."""
import argparse
import copy
import datetime
import json
import os
import re
import shutil
import subprocess
from pathlib import Path
from freeze import ROOT
from qualify import read,sha
from verify import need,parse_values,compare


def table(path, count):
    result={}
    for line in path.read_text('utf-8').splitlines():
        key,*values=line.split('\t');need(key not in result,'Duplicate observation');result[key]=values
    need(len(result)==count,'Observation floor differs');return result


def run(oracle):
    receipt=read(oracle/'oracle-result.json');need(receipt['original_JUnit_validation']['passed']==76,'Original JUnit is not qualified')
    out=oracle/('verification_'+datetime.datetime.now().strftime('%Y%m%d_%H%M%S_%f'));out.mkdir()
    combined=b''.join((oracle/name).read_bytes() for name in ['foundation-java.tsv','column-java.tsv','index-java.tsv'])
    (out/'source-java.tsv').write_bytes(combined);expected=[line.split('\t')[0] for line in combined.decode().splitlines()]
    need(len(expected)==len(set(expected))==57,'Wrong cumulative source case set');source= parse_values(out/'source-java.tsv',expected)
    column=table(oracle/'column-edges-java.tsv',21);index_all=table(oracle/'index-edges-java.tsv',148)
    identity={key:value for key,value in index_all.items() if key.startswith('enum-identity/')};need(len(identity)==4,'JVM identity inputs missing')
    index={key:value for key,value in index_all.items() if key not in identity};need(len(index)==144,'Computed results confused with identity inputs')
    inputs=[]
    for directory in ['crates','tools/formal-sqlparser-core','docs/phase-2/sqlparser_core']:
        inputs.extend(dict(path=path.relative_to(ROOT).as_posix(),sha256=sha(path)) for path in sorted((ROOT/directory).rglob('*')) if path.is_file())
    inputs.extend(dict(path=name,sha256=sha(ROOT/name)) for name in ['Cargo.toml','Cargo.lock'])
    base=os.environ|dict(CARGO_HOME='D:/codex-builds/seata-rust-cargo-home',CARGO_INCREMENTAL='0',TEMP='D:/Temp',TMP='D:/Temp')
    for name in ['SEATA_CORE_OBSERVATIONS','SEATA_COLUMN_EDGE_OBSERVATIONS','SEATA_INDEX_EDGE_OBSERVATIONS','SEATA_INDEX_IDENTITY_INPUTS','SEATA_INDEX_ORACLE_FIXTURE']:
        base.pop(name,None)
    commands=[]
    def invoke(argv,name,env,cwd=ROOT,failure=False):
        with (out/name).open('wb') as log:
            code=subprocess.run(list(map(str,argv)),cwd=cwd,env=env,stdout=log,stderr=subprocess.STDOUT,timeout=600,creationflags=subprocess.CREATE_NO_WINDOW).returncode
        commands.append(dict(argv=list(map(str,argv)),cwd=str(cwd),exit_code=code,log=name,
            env={key:env[key] for key in ['CARGO_HOME','CARGO_TARGET_DIR','RUSTC','RUSTUP_TOOLCHAIN','SEATA_CORE_OBSERVATIONS','SEATA_COLUMN_EDGE_OBSERVATIONS','SEATA_INDEX_EDGE_OBSERVATIONS','SEATA_INDEX_IDENTITY_INPUTS','SEATA_INDEX_ORACLE_FIXTURE'] if key in env}))
        (out/'commands.json').write_text(json.dumps(commands,indent=2)+'\n','utf-8')
        need(code==(101 if failure else 0),'Unexpected command exit: '+name);return (out/name).read_text('utf-8')
    results={}
    for version in ['1.99','1.85']:
        env=base|dict(CARGO_TARGET_DIR='D:/cargo-target/seata-rust-production'+('-msrv' if version=='1.85' else ''))
        cargo='cargo'
        if version=='1.85':
            binary=Path('C:/Users/hnxyh/.rustup/toolchains/1.85.0-x86_64-pc-windows-msvc/bin');cargo=binary/'cargo.exe'
            env.update(RUSTC=str(binary/'rustc.exe'),RUSTDOC=str(binary/'rustdoc.exe'),RUSTUP_TOOLCHAIN='1.85.0-x86_64-pc-windows-msvc',PATH=str(binary)+os.pathsep+env['PATH'])
        need('cargo '+version+'.' in invoke([cargo,'--version'],version+'-cargo.log',env),'Actual Cargo differs')
        invoke([env.get('RUSTC','rustc'),'--version'],version+'-rustc.log',env)
        if version=='1.99':invoke([cargo,'fmt','--all','--','--check'],version+'-fmt.log',env)
        for label,feature in [('default',[]),('all',['--all-features'])]:
            invoke([cargo,'check','--offline','--locked','--workspace','--all-targets']+feature,version+'-check-'+label+'.log',env)
        invoke([cargo,'clippy','--offline','--locked','--workspace','--all-features','--all-targets','--','-D','warnings'],version+'-clippy.log',env)
        env.update(SEATA_CORE_OBSERVATIONS=str(out/(version+'-source.tsv')),SEATA_COLUMN_EDGE_OBSERVATIONS=str(out/(version+'-column.tsv')),
            SEATA_INDEX_EDGE_OBSERVATIONS=str(out/(version+'-index.tsv')),SEATA_INDEX_IDENTITY_INPUTS=str(out/(version+'-identity-inputs.tsv')),
            SEATA_INDEX_ORACLE_FIXTURE=str(oracle/'index-edges-java.tsv'))
        log=invoke([cargo,'test','--offline','--locked','--workspace','--all-features','--all-targets'],version+'-tests.log',env)
        totals=[tuple(map(int,row)) for row in re.findall(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored',log)]
        need(sum(row[0] for row in totals)==434 and len(totals)==89 and sum(row[2] for row in totals)==143,'Regression/harness/ignored floor differs')
        for name in ['SEATA_CORE_OBSERVATIONS','SEATA_COLUMN_EDGE_OBSERVATIONS','SEATA_INDEX_EDGE_OBSERVATIONS','SEATA_INDEX_IDENTITY_INPUTS']:
            env.pop(name)
        doc=invoke([cargo,'test','--offline','--locked','--workspace','--all-features','--doc'],version+'-doc.log',env)
        need(sum(map(int,re.findall(r'test result: ok\. (\d+) passed;',doc)))==4,'Original doc floor differs')
        diff=compare(source,parse_values(out/(version+'-source.tsv'),expected));cdiff=compare(column,table(out/(version+'-column.tsv'),21));idiff=compare(index,table(out/(version+'-index.tsv'),144))
        need(table(out/(version+'-identity-inputs.tsv'),4)==identity,'Explicit identity input binding differs')
        for label,rows in [('source',diff),('column',cdiff),('index',idiff)]:
            (out/(version+'-'+label+'-diff.json')).write_text(json.dumps(rows,indent=2)+'\n','utf-8')
        results[version]=dict(ordinary=434,harness=89,ignored=143,doc=4,source_MATCH=57,computed_edge_MATCH=165,identity_inputs=4)
        print(json.dumps(dict(version=version,**results[version])),flush=True)
    # Isolated mutants exercise actual behavioral assertions; live source remains untouched.
    work=Path(receipt['work'])/'mutants'/out.name;need(work.resolve().is_relative_to(Path('D:/codex-builds').resolve()),'Mutation path escaped D')
    variants=[
        ('include-cardinality','struct/index_meta.rs','Ok(theirs.asc_or_desc == mine.asc_or_desc\n            && theirs.ordinal_position == mine.ordinal_position)',
         'Ok(theirs.asc_or_desc == mine.asc_or_desc && theirs.ordinal_position == mine.ordinal_position && theirs.cardinality == mine.cardinality)','index_meta_obligations','index_defaults_null_shortcuts_and_cardinality_contract'),
        ('coerce-null-indextype','struct/index_meta.rs',None,None,'index_meta_obligations','index_defaults_null_shortcuts_and_cardinality_contract'),
        ('cache-index-scalars-before-list','struct/index_meta.rs',None,None,'index_meta_obligations','index_reads_scalar_fields_after_list_comparison_callback'),
        ('make-fixed-list-growable','column_list.rs','Self::build(values, true)','Self::build(values, false)','index_java_diff','all_list_equals_modes_and_mutations_match_jdk17'),
        ('has-next-less-than','column_list.rs','Ok(cursor != self.len()?)','Ok(cursor < self.len()?)','index_java_diff','live_to_string_iteration_modes_match_jdk17'),
        ('eager-array-version-check','column_list.rs','for cursor in 0..mine.size {','for cursor in 0..mine.size { self.check_version(mine.modification)?; other.check_version(theirs.modification)?;','index_java_diff','all_list_equals_modes_and_mutations_match_jdk17'),
        ('swallow-object-equals-error','struct/column_meta.rs','left.java_equals(right.as_ref())?','left.java_equals(right.as_ref()).unwrap_or(false)','object_callback_errors','object_failures_preserve_identity_and_propagate_through_column_and_list'),
        ('swallow-cause-text-error','sql_parsing_exception.rs','.transpose()?','.transpose().unwrap_or(None)','object_callback_errors','throwing_cause_to_string_prevents_exception_construction_and_preserves_original_failure'),
        ('omit-enum-identity-hash','struct/index_meta.rs','state.indextype.map_or(0, enum_identity)','state.indextype.map_or(0, |_| 0)','index_java_diff','nullable_defaults_and_hash_replay_with_explicit_jvm_identity_match'),
    ]
    mutants=[]
    for name,relative,old,new,suite,test in variants:
        directory=work/name;directory.mkdir(parents=True)
        for crate in ['seata_sqlparser','seata_test']:shutil.copytree(ROOT/'crates'/crate,directory/'crates'/crate)
        (directory/'Cargo.toml').write_text('[workspace]\nmembers=["crates/seata_sqlparser","crates/seata_test"]\nresolver="2"\n[workspace.package]\nversion="0.1.0"\nedition="2021"\nrust-version="1.85"\nlicense="Apache-2.0"\nrepository="https://github.com/easy-4-rust/seata-rust"\n[workspace.lints.rust]\nunsafe_code="forbid"\n[workspace.lints.clippy]\nunwrap_used="warn"\nexpect_used="warn"\n','utf-8')
        path=directory/'crates/seata_sqlparser/src'/relative;text=path.read_text('utf-8');before=sha(path)
        if name=='coerce-null-indextype':
            for side in ['other','self']:
                anchor=f'.ok_or(MetadataError::NullReference("{side}.indextype"))?';need(text.count(anchor)==1,'Null anchor differs');text=text.replace(anchor,'.unwrap_or(IndexType::PRIMARY)')
        elif name=='cache-index-scalars-before-list':
            text=text.replace('let theirs = other.values()?;','let cached_theirs = other.capture()?; let cached_mine = self.capture()?; let theirs = other.values()?;')
            text=text.replace('let theirs = other.capture()?;\n        let mine = self.capture()?;','let theirs = cached_theirs; let mine = cached_mine;')
        else:
            need(text.count(old)==(2 if name=='eager-array-version-check' else 1),'Mutant anchor differs: '+name)
            text=text.replace(old,new,1)
        path.write_text(text,'utf-8')
        env=base|dict(CARGO_TARGET_DIR='D:/cargo-target/seata-index-mutants',SEATA_INDEX_ORACLE_FIXTURE=str(oracle/'index-edges-java.tsv'))
        log=invoke(['cargo','test','--offline','-p','seata-test','--test',suite,test,'--','--exact'],'mutant-'+name+'.log',env,directory,True)
        need('test result: FAILED. 0 passed; 1 failed;' in log and 'error[E' not in log,'Mutant must fail actual behavior, not compilation')
        mutants.append(dict(name=name,path=str(path),before_sha256=before,mutant_sha256=sha(path),result='RUNTIME_REJECTED'))
    controls=[]
    def reject(name,action):
        try:action()
        except (ValueError,KeyError):controls.append(name);return
        raise ValueError('Counterexample accepted: '+name)
    for label,baseline in [('source',source),('index',index)]:
        for mode in ['missing','foreign','mismatch']:
            changed=copy.deepcopy(baseline)
            if mode=='missing':changed.pop(next(iter(changed)))
            if mode=='foreign':changed['foreign']=['true']
            if mode=='mismatch':changed[next(iter(changed))]=['wrong']
            reject(label+'-'+mode,lambda:compare(baseline,changed))
    for row in inputs:need(sha(ROOT/row['path'])==row['sha256'],'Live input changed during verification')
    (out/'inputs.json').write_text(json.dumps(inputs,indent=2)+'\n','utf-8')
    (out/'identity-inputs.json').write_text(json.dumps(dict(role='INPUT_NOT_RESULT',bindings=identity),indent=2)+'\n','utf-8')
    result=dict(status='INDEX_METADATA_AND_TYPED_CALLBACKS_VERIFIED_CORE_OPEN',oracle=str(oracle),verification=str(out),
        source_cases_MATCH=57,source_cases_MISSING=19,object_states=dict(IMPLEMENTED=13,MISSING=18),
        original_JUnit=76,original_JUnit_controls=13,Rust=results,computed_index_edges_MATCH=144,previous_column_edges_MATCH=21,
        JVM_identity_inputs_not_MATCH=4,mutants=mutants,comparison_controls=controls,
        Rust_callback_API_change='JavaObject methods and JavaThrowable text/from_java_cause now return Result to preserve original thrown failures',
        full_core_complete=False,WP05_complete=False,WP06_complete=False,full_V1_complete=False,TC_safety_gate_passed=False,
        external_TC_risk_accepted=False,shared_lock_issue='deferred_by_user')
    (out/'result.json').write_text(json.dumps(result,indent=2)+'\n','utf-8')
    print(json.dumps(dict(verification=str(out),source_MATCH=57,computed_edges_MATCH=165,identity_inputs=4,mutants=len(mutants))),flush=True)


if __name__=='__main__':
    parser=argparse.ArgumentParser();parser.add_argument('--oracle',type=Path,required=True);run(parser.parse_args().oracle.resolve())
