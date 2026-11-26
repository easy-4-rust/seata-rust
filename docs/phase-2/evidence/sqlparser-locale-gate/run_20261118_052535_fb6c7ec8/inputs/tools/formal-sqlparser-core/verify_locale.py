"""Dual-toolchain Locale dependency gate; every original source completion denominator stays intact."""
import argparse
import copy
import datetime
import json
import os
import re
import shutil
import subprocess
from pathlib import Path
from freeze import ROOT,DOC
from qualify import read,sha
from verify import need,parse_values,compare
from verify_index import table

PARENT=ROOT/'docs/phase-2/evidence/sqlparser-table-prerequisites-gate/run_20261118_044535_5f39165d'


def expected(oracle):
    result={};inputs={}
    for line in (oracle/'jdk17-locale-observations.tsv').read_text('utf-8').splitlines():
        key,text,lower,upper,boundary=line.split('\t');need(key not in result,'Duplicate locale key')
        need(all(len(value)%4==0 for value in [text,lower,upper]) and len(boundary)==len(text)//4+1 and set(boundary)<=set('01'),'Malformed UTF16 observation')
        result[key]=[lower,upper,boundary];inputs[key]=text
    keys={f'case/{language}/{i}' for language in ['','en','tr','az','lt','th','el','xx'] for i in range(1548)}
    need(set(result)==keys and len(result)==12384,'Locale case scope differs')
    return result,inputs


def replace_body(text,anchor,body):
    need(text.count(anchor)==1,'Function anchor differs: '+anchor);start=text.index('{',text.index(anchor));depth=1;end=start+1
    while depth:
        need(end<len(text),'Unbalanced function body');depth+=(text[end]=='{')-(text[end]=='}');end+=1
    return text[:start+1]+body+text[end-1:]


def run(oracle):
    receipt=read(oracle/'oracle-result.json')
    need(sha(PARENT/'catalog.json')==receipt['parent_catalog_sha256']=='9df62e28460c66b2aae94071d417f2f10ce3c2a46eedf02af237b3daeb6845bb','Wrong parent')
    for row in read(PARENT/'catalog.json')['members']:need(sha(PARENT/row['path'])==row['sha256'],'Parent drift')
    need(len(receipt['original_runtime_inputs'])==219 and len(receipt['jdk_runtime_inputs'])==5,'Runtime scope differs')
    for name,digest in (receipt['original_runtime_inputs']|receipt['jdk_runtime_inputs']).items():need(sha(Path(name))==digest,'Runtime binding drift')
    for row in receipt['producer_sources']+receipt['producer_classes']:need(sha(Path(row['path']))==row['sha256'],'Actual producer drift')
    families=[line for line in (oracle/'profiles/probe.tsv').read_text('utf-8').splitlines() if line.startswith('sun.text.')]
    need(len(families)==2 and families[0].startswith('sun.text.RuleBasedBreakIterator:') and families[1].startswith('sun.text.DictionaryBasedBreakIterator:'),'Unexpected locale word profile family')
    for source in ['jdk17-string-casing-input.bin','jdk17-word-root-input.bin','jdk17-word-th-input.bin','jdk17-word-th-dictionary-input.bin']:
        need(sha(oracle/source)==sha(ROOT/'crates/seata_sqlparser/src/data'/source.replace('-','_')),'Generated runtime data drift')
    need(sha(ROOT/'Cargo.lock')==sha(PARENT/'inputs/Cargo.lock'),'Dependency lock changed')
    locale,_=expected(oracle);casing={key:value[:2] for key,value in locale.items()};boundary={key:value[2:] for key,value in locale.items()}
    index_gate=ROOT/read(PARENT/'result.json')['parent']
    source_keys={row['id'] for row in read(PARENT/'verification/1.99-source-diff.json')};source=parse_values(index_gate/'verification/source-java.tsv',source_keys)
    column=table(index_gate/'oracle/column-edges-java.tsv',21);all_index=table(index_gate/'oracle/index-edges-java.tsv',148)
    identity={key:value for key,value in all_index.items() if key.startswith('enum-identity/')};index={key:value for key,value in all_index.items() if key not in identity}
    prior_set=table(PARENT/'verification/1.99-table-set.tsv',23370)
    out=oracle/('verification_'+datetime.datetime.now().strftime('%Y%m%d_%H%M%S_%f'));out.mkdir()
    inputs=[]
    for directory in ['crates','tools/formal-sqlparser-core','docs/phase-2/sqlparser_core']:
        inputs.extend(dict(path=p.relative_to(ROOT).as_posix(),sha256=sha(p)) for p in sorted((ROOT/directory).rglob('*')) if p.is_file())
    inputs.extend(dict(path=name,sha256=sha(ROOT/name)) for name in ['Cargo.toml','Cargo.lock'])
    variables=['SEATA_CORE_OBSERVATIONS','SEATA_COLUMN_EDGE_OBSERVATIONS','SEATA_INDEX_EDGE_OBSERVATIONS','SEATA_INDEX_IDENTITY_INPUTS',
        'SEATA_INDEX_ORACLE_FIXTURE','SEATA_TABLE_SET_OBSERVATIONS','SEATA_LOCALE_CASING_OBSERVATIONS','SEATA_LOCALE_BOUNDARY_OBSERVATIONS']
    base=os.environ|dict(CARGO_HOME='D:/codex-builds/seata-rust-cargo-home',CARGO_INCREMENTAL='0',TEMP='D:/Temp',TMP='D:/Temp')
    for name in variables:base.pop(name,None)
    commands=[]
    def invoke(argv,name,env,cwd=ROOT,failure=False):
        with (out/name).open('wb') as log:code=subprocess.run(list(map(str,argv)),cwd=cwd,env=env,stdout=log,stderr=subprocess.STDOUT,timeout=600,creationflags=subprocess.CREATE_NO_WINDOW).returncode
        commands.append(dict(argv=list(map(str,argv)),cwd=str(cwd),exit_code=code,log=name,
            env={key:env[key] for key in variables+['CARGO_HOME','CARGO_TARGET_DIR','RUSTC','RUSTUP_TOOLCHAIN'] if key in env}))
        (out/'commands.json').write_text(json.dumps(commands,indent=2)+'\n','utf-8');need(code==(101 if failure else 0),'Unexpected exit: '+name)
        return (out/name).read_text('utf-8')
    results={}
    for version in ['1.99','1.85']:
        env=base|dict(CARGO_TARGET_DIR='D:/cargo-target/seata-rust-production'+('-msrv' if version=='1.85' else ''));cargo='cargo'
        if version=='1.85':
            binary=Path('C:/Users/hnxyh/.rustup/toolchains/1.85.0-x86_64-pc-windows-msvc/bin');cargo=binary/'cargo.exe'
            env.update(RUSTC=str(binary/'rustc.exe'),RUSTDOC=str(binary/'rustdoc.exe'),RUSTUP_TOOLCHAIN='1.85.0-x86_64-pc-windows-msvc',PATH=str(binary)+os.pathsep+env['PATH'])
        need('cargo '+version+'.' in invoke([cargo,'--version'],version+'-cargo.log',env),'Toolchain mismatch')
        invoke([env.get('RUSTC','rustc'),'--version'],version+'-rustc.log',env)
        if version=='1.99':invoke([cargo,'fmt','--all','--','--check'],version+'-fmt.log',env)
        for name,features in [('default',[]),('all',['--all-features'])]:invoke([cargo,'check','--offline','--locked','--workspace','--all-targets']+features,version+'-check-'+name+'.log',env)
        invoke([cargo,'clippy','--offline','--locked','--workspace','--all-features','--all-targets','--','-D','warnings'],version+'-clippy.log',env)
        for variable,label in zip(variables,['source','column','index','identity','fixture','table-set','locale-casing','locale-boundary']):
            env[variable]=str(index_gate/'oracle/index-edges-java.tsv') if label=='fixture' else str(out/(version+'-'+label+'.tsv'))
        log=invoke([cargo,'test','--offline','--locked','--workspace','--all-features','--all-targets'],version+'-tests.log',env)
        totals=[tuple(map(int,row)) for row in re.findall(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored',log)]
        need(len(totals)==91 and sum(r[0] for r in totals)==440 and sum(r[1] for r in totals)==0 and sum(r[2] for r in totals)==143,'Full regression floor differs')
        for name in variables:env.pop(name,None)
        log=invoke([cargo,'test','--offline','--locked','--workspace','--all-features','--doc'],version+'-doc.log',env)
        need(sum(map(int,re.findall(r'test result: ok\. (\d+) passed;',log)))==4,'Doc floor differs')
        for label,baseline,observed in [('source',source,parse_values(out/(version+'-source.tsv'),source_keys)),
            ('column',column,table(out/(version+'-column.tsv'),21)),('index',index,table(out/(version+'-index.tsv'),144)),
            ('table-set',prior_set,table(out/(version+'-table-set.tsv'),23370)),
            ('locale-casing',casing,table(out/(version+'-locale-casing.tsv'),12384)),('locale-boundary',boundary,table(out/(version+'-locale-boundary.tsv'),12384))]:
            (out/(version+'-'+label+'-diff.json')).write_text(json.dumps(compare(baseline,observed),indent=2)+'\n','utf-8')
        need(table(out/(version+'-identity.tsv'),4)==identity,'Prior identity input differs')
        results[version]=dict(ordinary=440,harness=91,ignored=143,doc=4,source_MATCH=57,inherited_computed_observations_MATCH=23535,
            locale_context_records_MATCH=12384,locale_result_fields_per_record=3)
        print(json.dumps(dict(version=version,**results[version])),flush=True)
    work=Path(receipt['work'])/'mutants'/out.name;need(work.resolve().is_relative_to(Path('D:/codex-builds').resolve()),'Mutation path escaped D')
    variants=[('truncate-root-expansion','jdk_casing.rs','output.extend_from_slice(&record.upper);','output.extend_from_slice(&record.upper[..1]);','contextual_string_lower_and_upper_match_original_jdk17'),
        ('erase-turkic-locale','jdk_casing.rs','matches!(language, "tr" | "az")','false','contextual_string_lower_and_upper_match_original_jdk17'),
        ('keep-lithuanian-dot','jdk_casing.rs','language == "lt" && cp == 0x307','false && language == "lt" && cp == 0x307','contextual_string_lower_and_upper_match_original_jdk17'),
        ('ignore-thai-profile','jdk_word_profile.rs','if language == "th" {','if false && language == "th" {','word_boundaries_match_original_root_and_thai_dictionary_profiles'),
        ('forward-only-boundaries','jdk_word_profile.rs',None,None,'word_boundaries_match_original_root_and_thai_dictionary_profiles'),
        ('precompute-final-sigma-boundaries','jdk_word_profile.rs',None,None,'contextual_string_lower_and_upper_match_original_jdk17'),
        ('previous-high-surrogate-lookahead','utf16_cursor.rs','u32::from(current)','self.current()','word_boundaries_match_original_root_and_thai_dictionary_profiles'),
        ('reject-empty-supplement-ranges','jdk_character_categories.rs','w[0] >> 8 > w[1] >> 8','w[0] >> 8 >= w[1] >> 8','word_boundaries_match_original_root_and_thai_dictionary_profiles'),
        ('host-unicode-uppercase','jdk_casing.rs',None,None,'contextual_string_lower_and_upper_match_original_jdk17')]
    mutants=[];surviving_variants=[]
    for name,relative,old,new,test in variants:
        directory=work/name;directory.mkdir(parents=True)
        for crate in ['seata_sqlparser','seata_test']:shutil.copytree(ROOT/'crates'/crate,directory/'crates'/crate)
        previous=read(PARENT/'result.json')['mutants'][0]['path'];(directory/'Cargo.toml').write_bytes((Path(previous).parents[3]/'Cargo.toml').read_bytes())
        path=directory/'crates/seata_sqlparser/src'/relative;text=path.read_text('utf-8');before=sha(path)
        if name=='forward-only-boundaries':
            body='let dictionary=if self.thai{Some(JdkWordDictionary::get()?)}else{None};let mut flags=vec![false;units.len()+1];flags[0]=true;let mut start=0;while start<units.len(){let(end,count)=self.next_range(units,start,dictionary)?;if count>1{if let Some(dictionary)=dictionary{for boundary in self.divide_dictionary(units,start,end,dictionary)?{flags[boundary]=true;}}}else{flags[end]=true;}start=end;}Ok(flags)'
            text=replace_body(text,'pub(crate) fn boundaries(',body)
        elif name=='precompute-final-sigma-boundaries':
            body='let flags=self.boundaries(units)?;let mut before=Utf16Cursor::new(units,index);while !flags[before.position]{let cp=before.previous();if cased(cp){let mut after=Utf16Cursor::new(units,index);after.position=after.next_index();while after.position<units.len()&&!flags[after.position]{if cased(after.current()){return Ok(false);}after.position=after.next_index();}return Ok(true);}}Ok(false)'
            text=replace_body(text,'pub(crate) fn is_final_cased(',body)
        elif name=='host-unicode-uppercase':text=replace_body(text,'pub fn upper_case(','Ok(JavaString::from(String::from_utf16_lossy(value.as_utf16()).to_uppercase()))')
        else:
            need(text.count(old)==(2 if name=='erase-turkic-locale' else 1),'Mutation anchor differs: '+name);text=text.replace(old,new)
        path.write_text(text,'utf-8')
        env=base|dict(CARGO_TARGET_DIR='D:/cargo-target/seata-locale-mutants')
        exploratory=name=='precompute-final-sigma-boundaries'
        log_name=('variant-' if exploratory else 'mutant-')+name+'.log'
        command=['cargo','test','--offline','-p','seata-test','--test','locale_casing',test,'--','--exact']
        if name=='previous-high-surrogate-lookahead':
            command=['cargo','test','--offline','-p','seata-sqlparser','--lib','utf16_cursor::tests::previous_preserves_code_point_before_partial_surrogate_position','--','--exact']
        log=invoke(command,log_name,env,directory,not exploratory)
        row=dict(name=name,path=str(path),before_sha256=before,mutant_sha256=sha(path))
        if exploratory:
            need('test result: ok. 1 passed; 0 failed;' in log and 'error[E' not in log,'Exploratory variant result changed; investigate counterexample')
            surviving_variants.append(row|dict(result='SURVIVED_NO_COUNTEREXAMPLE_FOUND',equivalence_proven=False,
                role='Exploratory implementation variation, not a rejected negative control; production retains original query order'))
        else:
            need('test result: FAILED. 0 passed; 1 failed;' in log and 'error[E' not in log,'Mutant must fail behavior')
            mutants.append(row|dict(result='RUNTIME_REJECTED'))
    controls=[]
    for label,baseline in [('source',source),('locale-casing',casing),('locale-boundary',boundary)]:
        for mode in ['missing','foreign','mismatch']:
            changed=copy.deepcopy(baseline)
            if mode=='missing':changed.pop(next(iter(changed)))
            if mode=='foreign':changed['foreign']=['true']
            if mode=='mismatch':changed[next(iter(changed))]=['wrong']
            try:compare(baseline,changed)
            except(ValueError,KeyError):controls.append(label+'-'+mode)
            else:raise ValueError('Accepted counterexample '+label+'-'+mode)
    for row in inputs:need(sha(ROOT/row['path'])==row['sha256'],'Live input drift')
    (out/'inputs.json').write_text(json.dumps(inputs,indent=2)+'\n','utf-8')
    result=dict(status='JDK_LOCALE_CASING_DEPENDENCY_VERIFIED_TABLE_META_OPEN',oracle=str(oracle),verification=str(out),Rust=results,
        source_object_states=dict(IMPLEMENTED=13,MISSING=18),original_case_states=dict(MATCH=57,MISSING=19),
        casing_property_inputs_not_MATCH=5004,word_profile_input_files_not_MATCH=3,locale_context_records_MATCH=12384,
        mutants=mutants,surviving_variants=surviving_variants,variant_equivalence_proven=False,comparison_controls=controls,registry_dependency_closure_changed=False,
        full_core_complete=False,WP05_complete=False,WP06_complete=False,full_V1_complete=False,TC_safety_gate_passed=False,
        external_TC_risk_accepted=False,shared_lock_issue='deferred_by_user',
        next_action='Integrate default Locale and metadata maps into TableMeta;19 original cases and18 source objects remain missing')
    (out/'result.json').write_text(json.dumps(result,indent=2)+'\n','utf-8')
    print(json.dumps(dict(verification=str(out),context_records_MATCH=12384,rejected_mutants=len(mutants),unclassified_surviving_variants=len(surviving_variants),source_MATCH=57,source_MISSING=19)),flush=True)


if __name__=='__main__':
    parser=argparse.ArgumentParser();parser.add_argument('--oracle',type=Path,required=True)
    run(parser.parse_args().oracle.resolve())
