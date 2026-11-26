"""Verify TableMeta's JDK comparator/set dependency without promoting the incomplete parent object."""
import argparse
import copy
import datetime
import json
import os
import re
import shutil
import subprocess
from pathlib import Path
from urllib.parse import unquote, urlparse
from freeze import ROOT, DOC, SOURCE
from qualify import read, sha
from verify import need, parse_values, compare
from verify_index import table

PARENT = ROOT/'docs/phase-2/evidence/sqlparser-index-gate/run_20261118_042124_511b04b6'


def expected_observations(oracle):
    result = {}
    for i,line in enumerate((oracle/'java17-comparisons.tsv').read_text('utf-8').splitlines()):
        fields = line.split('\t'); need(len(fields) == 3, 'Comparator fixture shape differs')
        result['comparison/'+str(i)] = [fields[2]]
    need(len(result) == 22492, 'Comparator denominator differs')
    for line in (oracle/'table-primary-sets.tsv').read_text('utf-8').splitlines():
        key,*values = line.split('\t'); need(key not in result, 'Duplicate set key'); result[key] = values
    need(len(result) == 23370, 'Set/comparator observation denominator differs')
    return result


def run(oracle):
    receipt = read(oracle/'oracle-result.json')
    need(sha(PARENT/'catalog.json') == receipt['parent_catalog_sha256'] == '6c44f71b71cf392a52fe6e9e4b8675af11da1cba70b5c1bddc4f0a8d57b69774', 'Wrong parent')
    for row in read(PARENT/'catalog.json')['members']: need(sha(PARENT/row['path']) == row['sha256'], 'Parent member drift')
    for name,digest in receipt['original_runtime_inputs'].items(): need(sha(Path(name)) == digest, 'Original runtime drift')
    need(len(receipt['jdk_runtime_inputs'])==5,'Actual JDK binding incomplete')
    for name,digest in receipt['jdk_runtime_inputs'].items(): need(sha(Path(name)) == digest, 'Actual JDK runtime drift')
    need(receipt['original_runtime_inputs'] == read(PARENT/'oracle/oracle-result.json')['original_runtime_inputs'], 'Original runtime scope differs')
    for row in receipt['producer_classes']: need(sha(Path(row['path'])) == row['sha256'], 'Actual producer class drift')
    need(sha(ROOT/'tools/formal-sqlparser-core/MetadataCaseOracle.java') == receipt['producer_source_sha256'] == sha(oracle/'MetadataCaseOracle.java'), 'Actual producer source drift')
    need(sha(ROOT/'crates/seata_sqlparser/src/data/java17_character_case.bin') == sha(oracle/'java17-character-case-input.bin'), 'Runtime mapping input drift')
    data = (oracle/'java17-character-case-input.bin').read_bytes()
    records = [tuple(int.from_bytes(data[i+j:i+j+4], 'big') for j in [0,4,8]) for i in range(0,len(data),12)]
    need(len(data) == 2799*12 and len(records) == 2799 and all(a[0] < b[0] for a,b in zip(records,records[1:])) and
         all(max(row) <= 0x10ffff and (row[0] != row[1] or row[0] != row[2]) for row in records), 'Malformed runtime mapping input')
    origin = (oracle/'case-origin.tsv').read_text('utf-8')
    class_name,uri=origin.splitlines()[0].split('\t')
    need(class_name=='org.apache.seata.sqlparser.struct.TableMeta' and Path(unquote(urlparse(uri).path).lstrip('/')).resolve()==
         (SOURCE/'sqlparser/seata-sqlparser-core/target/classes').resolve(), 'Original TableMeta code source differs')
    need(sha(ROOT/'Cargo.lock') == sha(PARENT/'inputs/Cargo.lock'), 'Dependency lock changed')
    expected = expected_observations(oracle)
    source_keys = {row['id'] for row in read(PARENT/'verification/1.99-source-diff.json')}
    source = parse_values(PARENT/'verification/source-java.tsv', source_keys)
    column = table(PARENT/'oracle/column-edges-java.tsv',21)
    index_all = table(PARENT/'oracle/index-edges-java.tsv',148)
    identity = {key:value for key,value in index_all.items() if key.startswith('enum-identity/')}
    index = {key:value for key,value in index_all.items() if key not in identity}
    out = oracle/('verification_'+datetime.datetime.now().strftime('%Y%m%d_%H%M%S_%f')); out.mkdir()
    inputs = []
    for directory in ['crates','tools/formal-sqlparser-core','docs/phase-2/sqlparser_core']:
        inputs.extend(dict(path=p.relative_to(ROOT).as_posix(),sha256=sha(p)) for p in sorted((ROOT/directory).rglob('*')) if p.is_file())
    inputs.extend(dict(path=name,sha256=sha(ROOT/name)) for name in ['Cargo.toml','Cargo.lock'])
    base = os.environ|dict(CARGO_HOME='D:/codex-builds/seata-rust-cargo-home',CARGO_INCREMENTAL='0',TEMP='D:/Temp',TMP='D:/Temp')
    variables = ['SEATA_CORE_OBSERVATIONS','SEATA_COLUMN_EDGE_OBSERVATIONS','SEATA_INDEX_EDGE_OBSERVATIONS','SEATA_INDEX_IDENTITY_INPUTS','SEATA_INDEX_ORACLE_FIXTURE','SEATA_TABLE_SET_OBSERVATIONS']
    for name in variables: base.pop(name,None)
    commands = []
    def invoke(argv,name,env,cwd=ROOT,failed=False):
        with (out/name).open('wb') as log:
            code = subprocess.run(list(map(str,argv)),cwd=cwd,env=env,stdout=log,stderr=subprocess.STDOUT,timeout=600,creationflags=subprocess.CREATE_NO_WINDOW).returncode
        commands.append(dict(argv=list(map(str,argv)),cwd=str(cwd),exit_code=code,log=name,
            env={key:env[key] for key in variables+['CARGO_HOME','CARGO_TARGET_DIR','RUSTC','RUSTUP_TOOLCHAIN'] if key in env}))
        (out/'commands.json').write_text(json.dumps(commands,indent=2)+'\n','utf-8')
        need(code == (101 if failed else 0),'Unexpected exit: '+name); return (out/name).read_text('utf-8')
    results = {}
    for version in ['1.99','1.85']:
        env = base|dict(CARGO_TARGET_DIR='D:/cargo-target/seata-rust-production'+('-msrv' if version=='1.85' else ''))
        cargo='cargo'
        if version=='1.85':
            binary=Path('C:/Users/hnxyh/.rustup/toolchains/1.85.0-x86_64-pc-windows-msvc/bin');cargo=binary/'cargo.exe'
            env.update(RUSTC=str(binary/'rustc.exe'),RUSTDOC=str(binary/'rustdoc.exe'),RUSTUP_TOOLCHAIN='1.85.0-x86_64-pc-windows-msvc',PATH=str(binary)+os.pathsep+env['PATH'])
        need('cargo '+version+'.' in invoke([cargo,'--version'],version+'-cargo.log',env), 'Toolchain mismatch')
        invoke([env.get('RUSTC','rustc'),'--version'],version+'-rustc.log',env)
        if version=='1.99':invoke([cargo,'fmt','--all','--','--check'],version+'-fmt.log',env)
        for name,features in [('default',[]),('all',['--all-features'])]:
            invoke([cargo,'check','--offline','--locked','--workspace','--all-targets']+features,version+'-check-'+name+'.log',env)
        invoke([cargo,'clippy','--offline','--locked','--workspace','--all-features','--all-targets','--','-D','warnings'],version+'-clippy.log',env)
        env.update(SEATA_CORE_OBSERVATIONS=str(out/(version+'-source.tsv')),SEATA_COLUMN_EDGE_OBSERVATIONS=str(out/(version+'-column.tsv')),
            SEATA_INDEX_EDGE_OBSERVATIONS=str(out/(version+'-index.tsv')),SEATA_INDEX_IDENTITY_INPUTS=str(out/(version+'-identity.tsv')),
            SEATA_INDEX_ORACLE_FIXTURE=str(PARENT/'oracle/index-edges-java.tsv'),SEATA_TABLE_SET_OBSERVATIONS=str(out/(version+'-table-set.tsv')))
        log=invoke([cargo,'test','--offline','--locked','--workspace','--all-features','--all-targets'],version+'-tests.log',env)
        totals=[tuple(map(int,row)) for row in re.findall(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored',log)]
        need(len(totals)==90 and sum(r[0] for r in totals)==437 and sum(r[2] for r in totals)==143, 'Regression floor differs')
        for name in variables:env.pop(name,None)
        log=invoke([cargo,'test','--offline','--locked','--workspace','--all-features','--doc'],version+'-doc.log',env)
        need(sum(map(int,re.findall(r'test result: ok\. (\d+) passed;',log)))==4,'Doc floor differs')
        for label,baseline,observed in [('source',source,parse_values(out/(version+'-source.tsv'),source_keys)),
            ('column',column,table(out/(version+'-column.tsv'),21)),('index',index,table(out/(version+'-index.tsv'),144)),
            ('table-set',expected,table(out/(version+'-table-set.tsv'),23370))]:
            diff=compare(baseline,observed);(out/(version+'-'+label+'-diff.json')).write_text(json.dumps(diff,indent=2)+'\n','utf-8')
        need(table(out/(version+'-identity.tsv'),4)==identity,'Prior identity inputs differ')
        results[version]=dict(ordinary=437,harness=90,ignored=143,doc=4,source_MATCH=57,prior_computed_edges_MATCH=165,
            table_set_computed_observations_MATCH=23370,character_mapping_inputs_not_MATCH=2799)
        print(json.dumps(dict(version=version,**results[version])),flush=True)
    work=Path(receipt['work'])/'mutants'/out.name
    variants=[
        ('host-unicode','java_case_mapping.rs',None,None,'all_java17_comparisons_match_including_raw_surrogates'),
        ('no-surrogate-lookbehind','java_string.rs','if (0xdc00..=0xdfff).contains(&current) && index > 0 {','if false && (0xdc00..=0xdfff).contains(&current) && index > 0 {','all_java17_comparisons_match_including_raw_surrogates'),
        ('replace-first-spelling','case_insensitive_pk_set.rs','// TreeMap替换value但不替换已存在的key，因而保留首次拼写。\n                return;','self.nodes[current].name = name; return;','original_table_primary_sets_match_order_representative_hash_and_errors'),
        ('case-normalized-hash','case_insensitive_pk_set.rs','sum.wrapping_add(name.java_hash_code())','sum.wrapping_add(JavaString::from(name.as_str().unwrap_or("").to_ascii_lowercase()).java_hash_code())','unmodifiable_empty_and_case_equality_keep_java_hash_inconsistency'),
        ('eager-null-contains','case_insensitive_pk_set.rs','if self.is_empty() {','if name.is_none() { return Err(MetadataError::NullReference("primary.contains.name")); } if self.is_empty() {','unmodifiable_empty_and_case_equality_keep_java_hash_inconsistency'),
        ('allow-empty-add-all','case_insensitive_pk_set.rs','Err(MetadataError::UnsupportedCollectionOperation("set.addAll"))','Ok(false)','unmodifiable_empty_and_case_equality_keep_java_hash_inconsistency'),
        ('revert-to-sorted-vector','case_insensitive_pk_set.rs',None,None,'original_table_primary_sets_match_order_representative_hash_and_errors')]
    mutants=[]
    for name,relative,old,new,test in variants:
        directory=work/name;directory.mkdir(parents=True)
        for crate in ['seata_sqlparser','seata_test']:shutil.copytree(ROOT/'crates'/crate,directory/'crates'/crate)
        (directory/'Cargo.toml').write_bytes((Path(read(PARENT/'result.json')['mutants'][0]['path']).parents[4]/'Cargo.toml').read_bytes())
        path=directory/'crates/seata_sqlparser/src'/relative;text=path.read_text('utf-8');before=sha(path)
        if name=='host-unicode':
            need(text.count('mapped(code_point, 4)')==text.count('mapped(code_point, 8)')==1,'Case mapping anchor differs')
            text=text.replace('mapped(code_point, 4)','char::from_u32(code_point).and_then(|ch| ch.to_uppercase().next()).map(u32::from).unwrap_or(code_point)')
            text=text.replace('mapped(code_point, 8)','char::from_u32(code_point).and_then(|ch| ch.to_lowercase().next()).map(u32::from).unwrap_or(code_point)')
        elif name=='revert-to-sorted-vector':text=Path('D:/Temp/seata-table-set-before-tree.rs').read_text('utf-8')
        else:need(text.count(old)==1,'Mutation anchor differs: '+name);text=text.replace(old,new)
        path.write_text(text,'utf-8')
        env=base|dict(CARGO_TARGET_DIR='D:/cargo-target/seata-table-set-mutants')
        log=invoke(['cargo','test','--offline','-p','seata-test','--test','table_primary_set',test,'--','--exact'], 'mutant-'+name+'.log',env,directory,True)
        need('test result: FAILED. 0 passed; 1 failed;' in log and 'error[E' not in log,'Mutant must fail behavior')
        mutants.append(dict(name=name,path=str(path),before_sha256=before,mutant_sha256=sha(path),result='RUNTIME_REJECTED'))
    controls=[]
    for mode in ['missing','foreign','mismatch']:
        changed=copy.deepcopy(expected)
        if mode=='missing':changed.pop(next(iter(changed)))
        if mode=='foreign':changed['foreign']=['true']
        if mode=='mismatch':changed[next(iter(changed))]=['wrong']
        try:compare(expected,changed)
        except (ValueError,KeyError):controls.append(mode)
        else:raise ValueError('Accepted counterexample: '+mode)
    for row in inputs:need(sha(ROOT/row['path'])==row['sha256'],'Live verification input drift')
    (out/'inputs.json').write_text(json.dumps(inputs,indent=2)+'\n','utf-8')
    result=dict(status='TABLE_PRIMARY_SET_DEPENDENCY_VERIFIED_PARENT_TABLE_META_OPEN',oracle=str(oracle),verification=str(out),Rust=results,
        original_case_states=dict(MATCH=57,MISSING=19),source_object_states=dict(IMPLEMENTED=13,MISSING=18),
        mutants=mutants,comparison_counterexamples=controls,character_mapping_role='2799_INPUTS_NOT_MATCH_RESULTS',
        original_runtime_inputs_bound=219,registry_dependency_closure_changed=False,original_source_changes=False,
        full_core_complete=False,WP05_complete=False,WP06_complete=False,full_V1_complete=False,
        TC_safety_gate_passed=False,external_TC_risk_accepted=False,shared_lock_issue='deferred_by_user',
        next_action='Implement TableMeta and original19 cases; Locale-sensitive maps/uppercase remain distinct from locale-independent comparator')
    (out/'result.json').write_text(json.dumps(result,indent=2)+'\n','utf-8')
    print(json.dumps(dict(verification=str(out),new_computed_observations_MATCH=23370,source_MATCH=57,source_MISSING=19,mutants=len(mutants))),flush=True)


if __name__ == '__main__':
    parser=argparse.ArgumentParser();parser.add_argument('--oracle',type=Path,required=True)
    run(parser.parse_args().oracle.resolve())
