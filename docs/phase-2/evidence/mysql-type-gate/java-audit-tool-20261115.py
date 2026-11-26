"""Independently recheck archived original source, classpath, loaded origins and generated golden bytes."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
from runtime_binding import RuntimeBinding

ROOT=Path(__file__).resolve().parents[2]
JAVA=ROOT.parent/'seata-java-phase09-oracle'


def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def read(path):return json.loads(path.read_text('utf-8'))
def need(value,message):
    if not value:raise ValueError(message)


def verify(build_dir,live_dir):
    build_dir=build_dir.resolve();live_dir=live_dir.resolve()
    for directory in [build_dir,live_dir]:
        for row in read(directory/'catalog.json')['members']:
            need(sha(directory/row['path'])==row['sha256'],'Sealed Java member differs')
    build=read(build_dir/'build.json');source_rows=read(build_dir/'source-inputs.json')
    sources={Path(x['path']).resolve():x for x in source_rows};pin=build['java_pin'];originals=[]
    for value in build['explicit_sources']:
        path=Path(value).resolve()
        if not path.is_relative_to(JAVA):continue
        raw=subprocess.check_output(['git','-C',str(JAVA),'show',pin+':'+path.relative_to(JAVA).as_posix()])
        archived=(build_dir/sources[path]['snapshot']).read_bytes()
        need(raw.replace(b'\r\n',b'\n')==archived.replace(b'\r\n',b'\n'),'Selected source differs beyond checkout line endings: '+str(path))
        originals.append(dict(path=str(path.relative_to(JAVA)),git_blob_sha256=hashlib.sha256(raw).hexdigest(),
            compiled_checkout_sha256=hashlib.sha256(archived).hexdigest(),exact_Git_blob=raw==archived,
            source_content_matches=True,checkout_line_endings_only=raw!=archived))
    need(len(originals)==18,'Original selected source denominator differs')
    link=read(live_dir/'build-binding.json')
    need(Path(link['path']).resolve()==build_dir and link['catalog_sha256']==sha(build_dir/'catalog.json'),'Live build link differs')
    binding=RuntimeBinding(build_dir,JAVA)
    need(binding.manifest()==read(live_dir/'runtime-inputs.json'),'Live runtime file manifest differs from build inputs')
    outcome=read(live_dir/'result.json');profiles=[];goldens=[]
    need(len(outcome['profiles'])==4 and not outcome['cleanup_errors'],'Missing profiles or cleanup')
    for profile in outcome['profiles']:
        label=profile['driver']+'-'+profile['profile'];directory=live_dir/label
        actual=binding.loaded(directory/'class-loading.log',profile['driver'],profile['succeeded'])
        need(actual==read(directory/'loaded-origin-binding.json'),'Recorded loaded origins do not match actual raw log')
        log=(directory/'stdout.log').read_bytes()
        if profile['driver']=='5.1.42':
            need(not profile['succeeded'] and b"Unknown system variable 'query_cache_size'" in log,'Wrong legacy failure')
        else:
            need(profile['succeeded'] and b'original_java_undo_cases=6 roundtrip=true' in log,'Original selected table cases not passed')
            observed=read(directory/'observations.json')
            need(observed['jackson']=='2.18.3' and observed['default_content_roundtrip'],'Wrong Jackson/default content')
            need((directory/'default-content.bin').read_bytes()==b'{}','Wrong original empty bytes')
            normal='org.apache.seata.rm.datasource.sql.struct.TableRecords';empty=normal+'$EmptyTableRecords'
            need(len(observed['cases'])==6,'Wrong table cases')
            for case in observed['cases']:
                need(case['original_roundtrip'] and case['before_runtime_classes']==[empty,normal,normal]
                    and case['after_runtime_classes']==[normal,normal,empty],'Original EmptyTableRecords class not retained')
            integer=next(x for x in observed['cases'] if x['table']=='undo_integer_types')
            signed=next(x for x in integer['values'][0] if x['name']=='boolish')
            unsigned=next(x for x in integer['values'][0] if x['name']=='boolish_unsigned')
            expected=('java.lang.Boolean','true',-7) if profile['profile']=='default' else ('java.lang.Integer','2',-6)
            need((signed['runtime_class'],signed['exact_value'],signed['jdbc_type'])==expected,'Wrong signed tinyint profile')
            need((unsigned['runtime_class'],unsigned['exact_value'],unsigned['jdbc_type'])==('java.lang.Integer','2',-6),'Unsigned tinyint identity differs')
            for source in directory.iterdir():
                if source.name=='observations.json' or source.name=='default-content.bin' or source.name.startswith('undo_') and source.suffix=='.json':
                    target=ROOT/'tests/fixtures/undo/java_8_0_27'/profile['profile']/source.name
                    need(target.read_bytes()==source.read_bytes(),'Generated golden bytes differ')
                    goldens.append(dict(source=str(source),target=str(target),sha256=sha(source)))
        profiles.append(dict(driver=profile['driver'],profile=profile['profile'],succeeded=profile['succeeded'],
            actual_loaded_file_classes=actual['file_classes'],all_loaded_origins_bound=True))
    binding.verify()
    return dict(status='selected_original_format_profile_verified',java_pin=pin,original_source_blobs=originals,
        build_catalog_sha256=sha(build_dir/'catalog.json'),live_catalog_sha256=sha(live_dir/'catalog.json'),
        profiles=profiles,actual_successful_original_table_cases=12,golden_assets=goldens,
        full_original_Maven_environment_recreated=False,Java_source_case_parity_complete=False,
        Rust_codec_interop_verified=False,SQL_Undo_atomicity_verified=False,WP05_complete=False,
        verifier_source_sha256=sha(Path(__file__)))


if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--build',required=True,type=Path);parser.add_argument('--live',required=True,type=Path)
    args=parser.parse_args();print(json.dumps(verify(args.build,args.live),ensure_ascii=False,indent=2))
