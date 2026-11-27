"""Bounded log adapter Rust gates; retain exact source inputs and command outputs."""
import argparse,datetime,json,os,subprocess,uuid,zipfile
from pathlib import Path
from inventory_rebuilt import ROOT,need,save,sha

def run(args):
    stamp=datetime.datetime.now().strftime('%Y%m%d_%H%M%S')+'_'+uuid.uuid4().hex[:8]
    out=ROOT/'docs/phase-2/evidence/structured-log-layer'/('red_' if args.red else 'mutation_' if args.expect_failure else 'rust_')/stamp
    out.mkdir(parents=True);print('STRUCTURED_LOG_LAYER '+str(out),flush=True)
    env=os.environ|dict(TEMP='D:/Temp',TMP='D:/Temp',CARGO_HOME=str(ROOT.parent/'.seata-cargo-cache'),CARGO_PROFILE_TEST_DEBUG='0',CARGO_INCREMENTAL='0',CARGO_TARGET_DIR=str(ROOT.parent/('.seata-target-nodebug' if args.rust=='1.99' else '.seata-target-nodebug-185')),SEATA_STRUCTURED_LOG_OUT=str(out/'observations'))
    sources=[]
    with zipfile.ZipFile(out/'source-inputs.zip','w',compression=zipfile.ZIP_DEFLATED) as archive:
        for p in sorted([p for p in (ROOT/'crates').rglob('*') if p.is_file()]+[ROOT/'Cargo.toml',ROOT/'Cargo.lock']):
            name=p.relative_to(ROOT).as_posix();archive.writestr(name,p.read_bytes());sources.append(dict(path=name,sha256=sha(p)))
    save(out/'source-inputs.json',sources)
    argv=['cargo','+'+args.rust+'.0','test','-p','seata-observability','--test','structured_log_layer','--features','structured_log_layer','--offline','--locked']
    if args.case:argv += [args.case,'--','--exact','--nocapture']
    value=subprocess.run(argv,cwd=ROOT,env=env,capture_output=True,timeout=300,creationflags=subprocess.CREATE_NO_WINDOW)
    (out/'tests.stdout').write_bytes(value.stdout);(out/'tests.stderr').write_bytes(value.stderr)
    save(out/'commands.json',[dict(argv=argv,exit_code=value.returncode)])
    need(value.returncode==(101 if args.red or args.expect_failure else 0),'Unexpected Rust target status: '+value.stderr[-1800:].decode(errors='replace'))
    if args.expect_failure:need(args.expect_failure.encode() in value.stdout+value.stderr,'Expected mutation failure absent')
    if args.red:need(b'Seata layer enabled a filtered application span' in value.stdout+value.stderr,'Behavioral RED marker absent')
    for row in sources:need(sha(ROOT/row['path'])==row['sha256'],'Executed source drift')
    save(out/'result.json',dict(status='LAYER_APPLICATION_ADMISSION_GAP_REPRODUCED' if args.red else 'SOURCE_MUTATION_REJECTED' if args.expect_failure else 'STRUCTURED_LOG_TARGETS_PASSED',Rust=args.rust,WP07_complete=False))
    (out/Path(__file__).name).write_bytes(Path(__file__).read_bytes())
    save(out/'catalog.json',[dict(path=p.relative_to(out).as_posix(),sha256=sha(p)) for p in sorted(out.rglob('*')) if p.is_file()])
    print(json.dumps(dict(evidence=str(out),catalog_sha256=sha(out/'catalog.json'))),flush=True)
if __name__=='__main__':
    parser=argparse.ArgumentParser();parser.add_argument('--rust',choices=['1.99','1.85'],required=True);parser.add_argument('--red',action='store_true');parser.add_argument('--case');parser.add_argument('--expect-failure');run(parser.parse_args())
