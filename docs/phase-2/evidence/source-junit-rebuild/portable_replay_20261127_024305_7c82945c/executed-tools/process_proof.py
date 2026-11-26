"""Actual parent/child JVM properties and untouched original pressure parameters."""
import copy,xml.etree.ElementTree as ET
from pathlib import Path
FASTJSON='org.apache.seata.rm.datasource.undo.parser.Fastjson2ConcurrentRefDeserializationTest'
def need(v,why):
    if not v:raise ValueError(why)
def read(path):return {e.attrib['key']:e.text for e in ET.parse(path).getroot().findall('entry')}
def validate(rows,module,runtime,work):
    need(rows and len({r['pid'] for r in rows})==len(rows),'JVM identity missing/duplicated')
    parents=[r for r in rows if r['sun.java.command'].startswith('SourceJUnitRunner ')]
    need(len(parents)==1,'Unique real engine parent required');parent=parents[0]
    need(parent['java.class.path']==runtime['classpath'] and parent['user.dir']==runtime['cwd'],'Actual parent command/classpath differs')
    fast=[r for r in rows if r['sun.java.command']==FASTJSON]
    need(len(fast)==(3 if module=='rm-datasource' else 0),'Original three pressure child JVMs missing')
    for row in rows:
        need(Path(row['user.home']).resolve()==(work/'home').resolve() and Path(row['java.io.tmpdir']).resolve()==Path('D:/Temp').resolve() and Path(row['jdk.net.hosts.file']).resolve()==(work/'hosts').resolve(),'Actual JVM environment escaped isolated D paths')
        need(row['seata.fastjson2.concurrentRef.rounds']=='<unset>','Original default rounds overridden')
        need(row['java.version']=='17.0.12' and int(row['pid'])>0,'Actual child JDK/PID differs')
        if row is not parent:
            need(row['parent_pid']==parent['pid'],'Foreign JVM substituted')
            need(row['sun.java.command']==FASTJSON or row['sun.java.command'].startswith('net.bytebuddy.agent.Attacher '),'Unexpected subprocess program')
    for row in fast:
        need(row['java.class.path']==parent['java.class.path'] and row['seata.fastjson2.concurrentRef.threads']=='200','Original child classpath/200-thread pressure weakened')
    return dict(actual_JVMs=len(rows),actual_pressure_children=len(fast),original_rounds=3 if fast else 0,original_threads_per_child=200 if fast else 0,environment_verified=True)
def controls(rows,runtime,work):
    cases=[]
    def reject(name,mutate):
        changed=copy.deepcopy(rows);mutate(changed)
        try:validate(changed,'rm-datasource',runtime,work)
        except (ValueError,KeyError,IndexError):cases.append(name);return
        raise ValueError('Weak process proof accepted '+name)
    def child(values):return next(v for v in values if v['sun.java.command']==FASTJSON)
    reject('child-home-on-C',lambda v:child(v).__setitem__('user.home','C:/unowned'))
    reject('child-temp-on-F',lambda v:child(v).__setitem__('java.io.tmpdir','F:/unowned'))
    reject('child-resolver-escaped',lambda v:child(v).__setitem__('jdk.net.hosts.file','C:/unowned/hosts'))
    reject('child-thread-pressure-lowered',lambda v:child(v).__setitem__('seata.fastjson2.concurrentRef.threads','199'))
    reject('original-rounds-lowered',lambda v:child(v).__setitem__('seata.fastjson2.concurrentRef.rounds','1'))
    reject('missing-original-child',lambda v:v.remove(child(v)))
    reject('child-classpath-truncated',lambda v:child(v).__setitem__('java.class.path','D:/unknown.jar'))
    reject('wrong-real-parent',lambda v:child(v).__setitem__('parent_pid','1'))
    return dict(count=len(cases),all_rejected=True,cases=cases)
