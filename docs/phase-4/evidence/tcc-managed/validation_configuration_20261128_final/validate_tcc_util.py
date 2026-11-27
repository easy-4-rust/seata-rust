"""Verify unchanged Java util execution, observed outlet reports and the Rust fixture bytes."""
import argparse
import copy
import json
from pathlib import Path
import subprocess
import zipfile
from inventory_rebuilt import ROOT,need,sha
from validate_structured_logs import catalog
from xa_managed_java_faults import guard

TRACKED='org.apache.seata.rm.tcc.api.BusinessActionContext$TrackedActionContextMap'
NAMES=['single_null','single_absent','batch_absent','batch_empty','same_pending','same_double','immediate_none','immediate_false','delayed','batch_merge','batch_nulls','object','array','object_string_same','rejected','report_null','report_false','report_true','report_rejected']
EXPECTED=[({},None,False,False),({},None,False,False),({},True,False,False),({},True,False,False),({'value':1},True,False,False),({'value':1.0},None,False,False),({'value':1},None,True,False),({'value':2},None,True,False),({'value':3},True,False,False),({'a':1,'b':False,'keep':7},None,True,False),({'a':1},True,False,False),({'value':'{"a":1}'},True,False,False),({'value':'[1,2]'},None,True,False),({'value':'{"a":1}'},None,False,False),({'value':4},True,False,True),({'a':1},None,False,False),({'a':1},False,False,False),({'a':1},None,True,False),({'a':1},True,False,True)]

def plain(values):
    need(values.get('@class')==TRACKED,'Actual original tracked map missing')
    return {k:v for k,v in values.items() if k!='@class'}

def equal_values(left,right):
    # Python 的 True==1 不能用于 Java JSON 类型相等性校验。
    return json.dumps(left,sort_keys=True,separators=(',',':'))==json.dumps(right,sort_keys=True,separators=(',',':'))

def semantic(rows):
    need([r['name'] for r in rows]==NAMES,'Java util matrix changed')
    count=0
    for row,(values,updated,result,failed) in zip(rows,EXPECTED,strict=True):
        need(row.get('updated') is updated and row['result'] is result and row['failed'] is failed and row['holder_cleared'] is True,'Java util result/dirty/clear boundary differs')
        need(equal_values(plain(row['values']),values),'Java util scalar/composite/null merge differs')
        need(row['reports'][0]=='java.util.ArrayList' and len(row['reports'])==2,'Java util outlet shape changed')
        reports=row['reports'][1];need(len(reports)==int(result or failed),'Java util same/delayed report sent or real report missing')
        for report in reports:
            need(report['@class']=='java.util.LinkedHashMap' and report['type']=='TCC' and report['status']=='Registered' and report['xid']=='original-xid' and report['branch_id']=='9007199254740995','Java util changed original report identity/type/status')
            app=json.loads(report['application_data'])
            need(set(app)=={'@class','actionContext'} and app['@class']=='java.util.Collections$SingletonMap' and equal_values(plain(app['actionContext']),values),'Java util reported partial/altered context')
            count+=1
    need(count==7,'Java util actual report invocation count differs')

def verify(folder,digest):
    catalog(folder,digest)
    result=json.loads((folder/'result.json').read_bytes())
    need(result['vectors']==19 and result['Java_pin']==guard.reg.PIN and result['Java_jar_sha256']==guard.reg.JAR_SHA and result['TCC_gate_complete'] is result['whole_plan_complete'] is False,'Java util scope/pin changed')
    sources={
        'BusinessActionContext.java':'integration-tx-api/src/main/java/org/apache/seata/rm/tcc/api/BusinessActionContext.java',
        'BusinessActionContextUtil.java':'integration-tx-api/src/main/java/org/apache/seata/rm/tcc/api/BusinessActionContextUtil.java',
        'ActionContextUtil.java':'integration-tx-api/src/main/java/org/apache/seata/integration/tx/api/interceptor/ActionContextUtil.java',
        'CommonFenceConstant.java':'integration-tx-api/src/main/java/org/apache/seata/integration/tx/api/fence/constant/CommonFenceConstant.java',
        'DefaultResourceManager.java':'rm/src/main/java/org/apache/seata/rm/DefaultResourceManager.java',
        'SpringFenceHandler.java':'spring/seata-spring/src/main/java/org/apache/seata/rm/fence/SpringFenceHandler.java',
    }
    for name,path in sources.items():
        original=subprocess.check_output(['git','-C',str(guard.reg.ORACLE),'show',guard.reg.PIN+':'+path])
        need((folder/name).read_bytes()==original,'Original Java util source modified')
    need((folder/'UtilOracle.java').read_bytes()==(ROOT/'tools/tcc/java/UtilOracle.java').read_bytes(),'Executed util wrapper differs')
    commands=json.loads((folder/'commands.json').read_bytes());need(len(commands)==2 and all(c['exit_code']==0 for c in commands),'Java util compile/execution failed')
    compiled=commands[0]['argv'];need(all(str(name) in str(compiled) for name in list(sources)[:-1]),'Original util/report dispatcher not compiled')
    loaded=(folder/'class-load.log').read_bytes().splitlines()
    for name in ['org.apache.seata.rm.tcc.api.BusinessActionContext','org.apache.seata.rm.tcc.api.BusinessActionContextUtil','org.apache.seata.rm.DefaultResourceManager']:
        need(any((name+' source:').encode() in r and b'/classes/' in r for r in loaded),'Original util/dispatcher not loaded from compiled source')
    jar=guard.reg.ORACLE/'server/target/seata-server.jar';need(sha(jar)==guard.reg.JAR_SHA,'Pinned Java server artifact changed')
    with zipfile.ZipFile(jar) as archive:
        pinned={Path(n).name:__import__('hashlib').sha256(archive.read(n)).hexdigest() for n in archive.namelist() if n.startswith('BOOT-INF/lib/') and n.endswith('.jar')}
    for module in ['integration-tx-api','rm']:
        binary=guard.reg.ORACLE/module/'target'/('seata-'+module+'-2.8.0-SNAPSHOT.jar');pinned[binary.name]=sha(binary)
    for row in json.loads((folder/'dependencies.json').read_bytes()):need(pinned.get(row['name'])==row['sha256'],'Util dependency not pinned artifact')
    rows=[json.loads(line.split(b'\t',1)[1]) for line in (folder/'oracle.stdout').read_bytes().splitlines() if line.startswith(b'UTIL\t')]
    fixture=ROOT/result['fixture'];need(sha(fixture)==result['fixture_sha256'] and json.loads(fixture.read_bytes())==rows,'Rust util fixture not actual Java output')
    semantic(rows);return rows

def weakenings(rows):
    rejected=0
    for i,row in enumerate(rows):
        variants=[]
        for field in ['updated','result','failed','values','reports']:
            changed=copy.deepcopy(rows)
            if field=='updated':changed[i][field]=False if row.get(field) is not False else None
            elif field in ['result','failed']:changed[i][field]=not row[field]
            elif field=='values':changed[i][field]['value']='altered'
            else:changed[i][field][1]=[] if row[field][1] else [{}]
            variants.append(changed)
        if row['reports'][1]:
            for field in ['status','type','xid','branch_id','application_data']:
                changed=copy.deepcopy(rows);changed[i]['reports'][1][0][field]='altered';variants.append(changed)
        for changed in variants:
            try:semantic(changed)
            except (ValueError,KeyError):rejected+=1
            else:raise RuntimeError('Util evidence weakening accepted')
    need(rejected==130,'Util weakening matrix incomplete');return rejected

if __name__=='__main__':
    parser=argparse.ArgumentParser();parser.add_argument('folder',type=Path);parser.add_argument('catalog_sha256');parser.add_argument('--weakenings',action='store_true');args=parser.parse_args()
    rows=verify(args.folder,args.catalog_sha256)
    if args.weakenings:print('Rejected Java util evidence weakenings:',weakenings(rows))
    print('Verified 19 original Java util observations and seven real dispatcher outlet invocations; network authority is separate')
