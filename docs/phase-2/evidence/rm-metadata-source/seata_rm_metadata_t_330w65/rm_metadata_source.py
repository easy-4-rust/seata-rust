"""Execute four pinned RM metadata JUnit classes using byte-verified and portable inputs."""
import argparse,base64,collections,hashlib,json,os,re,shutil,subprocess,tempfile,urllib.parse
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
HERE=Path(__file__).resolve().parent
ORACLE=ROOT.parent/"seata-java-phase09-oracle"
PIN="cba73e180fd8896a99df6415b859d275122feaa7"
BASE=ROOT/"docs/phase-2/evidence/sqlparser-rm-jdbc-source/seata_jdbc_source_5oazvawu"
CATALOG="42eb747c3ae8bbcff58739f5b73980cb19d0dfbf37e3bb290823082c0630193d"
JDK=Path("D:/Java/jdk-17.0.12/bin")
CLASSES=("ColumnMetaTest","IndexMetaTest","IndexTypeTest","TableMetaTest")
PREFIX="org.apache.seata.rm.datasource.sql.struct."
def sha(data):return hashlib.sha256(data).hexdigest()
def dump(path,value):path.write_text(json.dumps(value,ensure_ascii=False,indent=2)+"\n",encoding="utf-8")
def run(portable=None):
    work=Path(tempfile.mkdtemp(prefix="seata_rm_metadata_",dir="D:/codex-builds"))
    out=ROOT/"docs/phase-2/evidence/rm-metadata-source"/work.name
    out.mkdir(parents=True)
    for name in ("cp","main","original","observed","helper","source","home"):
        (work/name).mkdir()
    print("RM_METADATA_SOURCE "+str(out),flush=True)
    env=os.environ|dict(TEMP="D:/Temp",TMP="D:/Temp")
    for key in ("CLASSPATH","JAVA_TOOL_OPTIONS","JDK_JAVA_OPTIONS","_JAVA_OPTIONS"):
        env.pop(key,None)
    commands=[]
    def command(label,args,expect=0):
        args=list(map(str,args))
        v=subprocess.run(args,cwd=work,env=env,capture_output=True,timeout=120,creationflags=subprocess.CREATE_NO_WINDOW)
        (out/(label+".stdout")).write_bytes(v.stdout);(out/(label+".stderr")).write_bytes(v.stderr)
        commands.append(dict(label=label,argv=args,exit=v.returncode))
        dump(out/"commands.json",commands)
        assert v.returncode==expect,(label,v.stderr[-1800:].decode(errors="replace"))
        return v.stdout
    candidates={}
    if portable:
        original=json.loads((portable/"portable-manifest.json").read_bytes())
        for row in original["members"]:
            data=(portable/row["snapshot"]).read_bytes()
            assert sha(data)==row["sha256"]
            target=work/row["path"];target.parent.mkdir(parents=True,exist_ok=True);target.write_bytes(data)
            candidates[row["path"]]=row["sha256"]
        dump(out/"bootstrap.json",dict(mode="portable",source=str(portable),members=len(candidates),manifest_sha256=sha((portable/"portable-manifest.json").read_bytes())))
    else:
        assert sha((BASE/"catalog.json").read_bytes())==CATALOG
        catalog={r["path"]:r["sha256"] for r in json.loads((BASE/"catalog.json").read_bytes())["members"]}
        data=(BASE/"classpath-inputs.json").read_bytes()
        assert sha(data)==catalog["classpath-inputs.json"]
        rows=json.loads(data)
        mapping=collections.defaultdict(list)
        files=command("existing-class-files",["rg","--files","--hidden","--no-ignore",ORACLE,"-g","*.class"]).decode().splitlines()
        for name in files:
            path=Path(name)
            for marker in ("/target/classes/","/target/test-classes/"):
                if marker in path.as_posix():mapping[path.as_posix().split(marker,1)[1]].append(path)
        jars=command("existing-public-jars",["rg","--files","--hidden","--no-ignore","C:/Users/hnxyh/.m2/repository","-g","*.jar"]).decode().splitlines()
        for name in jars:mapping[Path(name).name].append(Path(name))
        recovered=[];missing=[]
        for row in rows:
            suffix="/".join(row["path"].split("/")[2:])
            options=mapping[Path(suffix).name] if suffix.endswith(".jar") else mapping[suffix]
            match=next((p for p in options if sha(p.read_bytes())==row["sha256"]),None)
            if match is None:missing.append(row["path"]);continue
            target=work/row["path"];target.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(match,target)
            candidates[row["path"]]=row["sha256"]
            recovered.append(dict(path=row["path"],sha256=row["sha256"],recovered_from=str(match)))
        dump(out/"bootstrap.json",dict(mode="verified-existing-inputs",baseline_catalog_sha256=CATALOG,original_members=len(rows),recovered=recovered,missing=missing,full_baseline_restored=False))
    slots={}
    for name in candidates:
        p=Path(name)
        slot=p.parts[1]
        slots[int(slot)]=work/p if p.suffix==".jar" else work/"cp"/slot
    cp=os.pathsep.join(str(slots[k]) for k in sorted(slots))
    production=[];tests=[];source_rows=[];expected={}
    for leaf in ("ColumnMeta","IndexMeta","IndexType","TableMeta"):
        relative=f"sqlparser/seata-sqlparser-core/src/main/java/org/apache/seata/sqlparser/struct/{leaf}.java"
        production.append(relative)
    for leaf in CLASSES:
        relative=f"rm-datasource/src/test/java/org/apache/seata/rm/datasource/sql/struct/{leaf}.java"
        tests.append(relative)
    for relative in production+tests:
        data=command("git-"+Path(relative).stem,["git","-C",ORACLE,"cat-file","blob",PIN+":"+relative])
        blob=subprocess.check_output(["git","-C",str(ORACLE),"rev-parse",PIN+":"+relative]).decode().strip()
        assert hashlib.sha1(b"blob "+str(len(data)).encode()+b"\0"+data).hexdigest()==blob
        target=work/"source"/relative;target.parent.mkdir(parents=True,exist_ok=True);target.write_bytes(data)
        snapshot=out/"sources"/relative;snapshot.parent.mkdir(parents=True,exist_ok=True);snapshot.write_bytes(data)
        source_rows.append(dict(path=relative,git_blob=blob,sha256=sha(data)))
        if relative in tests:
            text=data.decode("utf-8")
            methods=re.findall(r"@Test\s+public void (\w+)\(\)\s*\{",text)
            assert methods
            parts=re.split(r"@Test\s+public void \w+\(\)\s*\{",text)
            for method,body in zip(methods,parts[1:],strict=True):
                expected[PREFIX+Path(relative).stem+"#"+method]=len(re.findall(r"Assertions\.assert\w+\(",body))
            modified=data.replace(b"Assertions.",b"seata.verification.RmMetadataAssertions.")
            assert modified!=data
            observed=work/"source/observed"/Path(relative).name;observed.parent.mkdir(exist_ok=True);observed.write_bytes(modified)
    assert len(expected)==10
    dump(out/"source-inputs.json",dict(pin=PIN,files=source_rows,original_cases=expected))
    def javac(label,destination,paths,classpath,source8=False):
        args=["-encoding","UTF-8","-sourcepath","","-cp",classpath,"-d",str(destination)]
        if source8:args+=["-source","8","-target","8","-Xlint:-options"]
        args+=list(map(str,paths))
        file=work/(label+".args")
        file.write_text("\n".join('"'+a.replace("\\","/").replace('"','\\"')+'"' for a in args),encoding="utf-8")
        command(label,[JDK/"javac.exe","@"+str(file)])
        shutil.copyfile(file,out/(label+".args"))
    javac("compile-pinned-main",work/"main",[work/"source"/p for p in production],cp,True)
    javac("compile-helpers",work/"helper",[HERE/"RmMetadataAssertions.java",HERE/"RmMetadataJUnit.java"],cp)
    current=os.pathsep.join((str(work/"main"),str(work/"helper"),cp))
    javac("compile-original-tests",work/"original",[work/"source"/p for p in tests],current,True)
    javac("compile-observed-tests",work/"observed",sorted((work/"source/observed").glob("*.java")),current,True)
    def junit(label,directory):
        classpath=os.pathsep.join((str(directory),str(work/"main"),str(work/"helper"),cp))
        # The classpath can exceed the Windows command line limit, so java also uses an argfile.
        args=["-Dfile.encoding=UTF-8","-Duser.language=en","-Duser.country=US","-Duser.timezone=UTC",
              "-Duser.home="+str(work/"home"),"-Djava.io.tmpdir=D:/Temp",
              "-Xlog:class+load=info:file="+str(work/(label+"-classes.log")).replace("\\","/"),
              "-cp",classpath,"seata.verification.RmMetadataJUnit",str(out/(label+"-events.jsonl")),*[PREFIX+c for c in CLASSES]]
        file=work/(label+".args");file.write_text("\n".join('"'+a.replace("\\","/")+'"' for a in args),encoding="utf-8")
        stdout=command(label,[JDK/"java.exe","@"+str(file)])
        rows=[json.loads(line) for line in (out/(label+"-events.jsonl")).read_text().splitlines()]
        assert rows[-1]==dict(kind="summary",passed=10,failed=0,skipped=0)
        cases=[r for r in rows if r["kind"]=="case"]
        assert {r["class"]+"#"+r["method"] for r in cases}==set(expected)
        assert len({r["case_id"] for r in cases})==10 and all(r["status"]=="SUCCESSFUL" for r in cases)
        shutil.copyfile(work/(label+"-classes.log"),out/(label+"-classes.log"))
        return stdout,rows
    _,original=junit("original-junit",work/"original")
    observed_stdout,_=junit("observed-junit",work/"observed")
    assertions=collections.defaultdict(list)
    for line in observed_stdout.decode("utf-8").splitlines():
        if line.startswith("RM_METADATA_ASSERT\t"):
            _,case,value=line.split("\t")
            assertions[case].append(base64.b64decode(value).decode())
    assert {case:len(values) for case,values in assertions.items()}==expected
    dump(out/"java-assertions.json",assertions)
    # Qualify actual class loading, then retain only loaded sealed classes and whole loaded public JARs.
    used={};origins={}
    for label in ("original-junit","observed-junit"):
        text=(out/(label+"-classes.log")).read_text(encoding="utf-8")
        for line in text.splitlines():
            match=re.search(r"\[class,load\] (\S+) source: (file:\S+)",line)
            if not match:continue
            name,url=match.groups()
            raw=urllib.parse.unquote(urllib.parse.urlsplit(url).path)
            if re.match(r"^/[A-Za-z]:",raw):raw=raw[1:]
            path=Path(raw)
            if name.startswith(PREFIX) and name.endswith("Test"):
                assert path.resolve()==(work/("original" if label=="original-junit" else "observed")).resolve()
                origins[label+"#"+name]=str(path)
            if name in ["org.apache.seata.sqlparser.struct."+x for x in ("ColumnMeta","IndexMeta","IndexType","TableMeta")]:
                assert path.resolve()==(work/"main").resolve()
                origins[label+"#"+name]=str(path)
            if path.is_relative_to(work/"cp"):
                member=path if path.is_file() else path/(name.replace(".","/")+".class")
                assert member.is_file(),(name,path)
                relative=member.relative_to(work).as_posix()
                assert sha(member.read_bytes())==candidates[relative]
                used[relative]=member
    assert len(origins)==16
    for name in candidates:
        path=Path(name)
        if path.suffix not in (".class",".jar") and any(p.parent==work/path.parent for p in used.values()):
            used[name]=work/path
    # Runtime loading omits types used only by signatures in javac (for example ColumnUtils).
    compile_dependencies=set()
    for relative in production:
        text=(work/"source"/relative).read_text(encoding="utf-8")
        compile_dependencies.update(name.replace(".","/")+".class" for name in re.findall(r"^import (org\.apache\.seata\.[\w.]+);",text,re.MULTILINE))
    for dependency in compile_dependencies:
        matches=[name for name in candidates if "/".join(Path(name).parts[2:])==dependency]
        assert len(matches)==1,dependency
        used[matches[0]]=work/matches[0]
    portable_rows=[]
    for i,(name,path) in enumerate(sorted(used.items())):
        data=path.read_bytes();snapshot=f"portable-inputs/{i:03d}.bin";destination=out/snapshot;destination.parent.mkdir(exist_ok=True);destination.write_bytes(data)
        portable_rows.append(dict(path=name,snapshot=snapshot,sha256=sha(data)))
    dump(out/"portable-manifest.json",dict(members=portable_rows,only_selected_runtime=True,full_1079_input_baseline=False))
    dump(out/"class-origins.json",origins)
    compiled=[]
    for role in ("main","original","observed","helper"):
        for path in sorted((work/role).rglob("*.class")):
            data=path.read_bytes();snapshot="compiled/"+role+"/"+path.relative_to(work/role).as_posix()+".bin"
            target=out/snapshot;target.parent.mkdir(parents=True,exist_ok=True);target.write_bytes(data)
            compiled.append(dict(role=role,path=path.relative_to(work/role).as_posix(),sha256=sha(data),snapshot=snapshot))
    dump(out/"compiled-bindings.json",compiled)
    for path in (HERE/"RmMetadataJUnit.java",HERE/"RmMetadataAssertions.java",Path(__file__)):
        shutil.copyfile(path,out/path.name)
    dump(out/"result.json",dict(work=str(work),pin=PIN,original_cases=10,original_assertions=sum(expected.values()),java_assertions_recorded=sum(map(len,assertions.values())),original_sources_changed=False,observed_copy_only=True,portable_members=len(used),Rust_parity_complete=False,full_baseline_restored=False,WP05_complete=False))
    print("PASS four pinned original classes, ten original JUnit cases; "+str(out),flush=True)
    return out
if __name__=="__main__":
    parser=argparse.ArgumentParser();parser.add_argument("--portable",type=Path)
    args=parser.parse_args();run(args.portable)
