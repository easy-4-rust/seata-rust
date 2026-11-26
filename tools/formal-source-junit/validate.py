"""Independent source-method and final JUnit-plan closure; green Java is not Rust parity."""
import copy,json,collections
from pathlib import Path
MODULES={'seata-sqlparser-core':(76,0),'rm-datasource':(1212,39)}
DISABLED={'org.apache.seata.rm.datasource.ConnectionProxyTest','org.apache.seata.rm.datasource.exec.AbstractDMLBaseExecutorTest'}
def need(value,why):
    if not value:raise ValueError(why)
def load(path):return [json.loads(line) for line in path.read_bytes().splitlines()]
def validate_events(rows,module):
    passed,disabled_count=MODULES[module]
    need(rows and rows[0]['kind']=='runtime' and rows[-1]['kind']=='summary','Incomplete real engine transcript')
    need([r['sequence'] for r in rows]==list(range(1,len(rows)+1)),'Event sequence missing/duplicated')
    allowed={'runtime','summary','discovered','dynamic_registered','final_descriptor','started','finished','skipped','plan_finished'}
    need(all(r['kind'] in allowed for r in rows) and sum(r['kind']=='runtime' for r in rows)==sum(r['kind']=='summary' for r in rows)==1,'Unknown/duplicate original engine event')
    runtime=rows[0];summary=rows[-1]
    need(runtime['mode']==summary['mode']=='execute' and runtime['java_version']=='17.0.12' and runtime['jupiter_version']=='5.8.2','Actual engine/compiler profile differs')
    need('/junit-jupiter-engine/5.8.2/' in runtime['jupiter_origin'].replace('\\','/') and '/junit-platform-launcher/1.8.2/' in runtime['launcher_origin'].replace('\\','/'),'Actual original engine origin differs')
    need(summary['passed']==passed and summary['failed']==summary['container_failed']==summary['aborted']==summary['guard_denials']==0,'Failed/aborted/guarded run cannot qualify')
    def index(kind):
        selected=[r for r in rows if r['kind']==kind];need(len({r['id'] for r in selected})==len(selected),'Duplicate '+kind+' identity');return {r['id']:r for r in selected}
    discovered=index('discovered');dynamic=index('dynamic_registered');final=index('final_descriptor');started=index('started');finished=index('finished');skipped=index('skipped')
    need(not(set(discovered)&set(dynamic)) and set(discovered)|set(dynamic)==set(final),'Final original plan does not close discovery/dynamic identities')
    need(set(started)==set(finished) and set(started).issubset(final) and set(skipped).issubset(final) and all(v['status']=='SUCCESSFUL' for v in finished.values()),'Original execution missing, foreign or failed')
    disabled_nodes={k for k in final if any(k==s or k.startswith(s+'/') for s in skipped)}
    need(not set(finished)&disabled_nodes and set(final)==set(finished)|disabled_nodes,'Whole original plan container/test lifecycle is incomplete')
    end=[r for r in rows if r['kind']=='plan_finished'];need(len(end)==1 and end[0]['sequence']==rows[-2]['sequence'],'Actual plan end missing or duplicated')
    need(all(r['sequence']<end[0]['sequence'] for r in [*started.values(),*finished.values(),*final.values(),*skipped.values()]),'Lifecycle after original plan end')
    for identity,node in final.items():
        parent=node.get('parent');need(parent is None or parent in final and identity.startswith(parent+'/'),'Original parent identity detached')
        if identity in started:
            need(started[identity]['sequence']<finished[identity]['sequence'],'Result precedes start')
        for kind in [discovered,dynamic,started,finished,skipped]:
            if identity in kind:
                need(all(kind[identity].get(k)==node.get(k) for k in ['test','container','class','method','declaring_class','parameters','parent']),'Original case/source metadata differs')
    leaf={k for k,v in final.items() if v['test']};actual={k for k,v in finished.items() if v['test']}
    disabled={k for k in leaf if any(k==s or k.startswith(s+'/') for s in skipped)}
    need(not actual&disabled and leaf==actual|disabled and len(actual)==passed and len(disabled)==disabled_count,'Concrete source cases missing from completion/disabled partition')
    need({r.get('class') for r in skipped.values()}==(DISABLED if module=='rm-datasource' else set()),'Unexpected source skip or hidden case')
    need(all(r['reason']=='Disabled on JRE version: 17.0.12' for r in skipped.values()),'Source disabled reason changed')
    cases=[]
    for identity in sorted(leaf):
        node=final[identity];source=node
        while 'method' not in source:
            parent=source.get('parent');need(parent in final,'Concrete source case has no declaring method');source=final[parent]
        cases.append(dict(module=module,case_id=identity,display=node['display'],declaring_class=source['declaring_class'],method=source['method'],parameters=source['parameters'],Java_status='PASSED' if identity in actual else 'SOURCE_DISABLED_JDK17',Rust_disposition='MISSING',differential='NOT_RUN'))
    return dict(passed=passed,disabled_cases=disabled_count,dynamic_registered=len(dynamic),cases=cases)
def bind_methods(declarations,case_groups):
    keys=[(r['declaring_class'],r['method']) for r in declarations];need(len(keys)==1135 and len(set(keys))==1135,'Original declaration floor differs or overload unresolved')
    grouped=collections.defaultdict(list)
    for group in case_groups:
        for case in group['cases']:
            key=(case['declaring_class'],case['method']);need(key in set(keys),'Runtime method missing from original AST');grouped[key].append(case)
    rows=[]
    for source in declarations:
        cases=grouped.get((source['declaring_class'],source['method']),[]);need(cases,'Original method has no executed or explicitly disabled source cases')
        statuses={c['Java_status'] for c in cases};need(len(statuses)==1,'Mixed source execution/disabled status for one declaration')
        rows.append(source|dict(concrete_cases_expanded=next(iter(statuses))=='PASSED',Java_status=next(iter(statuses)),concrete_case_ids=[c['module']+'::'+c['case_id'] for c in cases],Rust_disposition='MISSING',differential='NOT_RUN'))
    need(sum(r['Java_status']=='PASSED' for r in rows)==1096 and sum(r['Java_status']=='SOURCE_DISABLED_JDK17' for r in rows)==39,'Original method execution denominator differs')
    return rows
def controls(rows,module):
    cases=[]
    def reject(name,changed):
        # Re-numbering defeats a superficial sequence-only rejection; structure must still reject.
        for i,r in enumerate(changed):r['sequence']=i+1
        try:validate_events(changed,module)
        except (ValueError,KeyError,IndexError):cases.append(name);return
        raise ValueError('Counterexample accepted: '+name)
    changed=copy.deepcopy(rows);i=next(i for i,r in enumerate(changed) if r['kind']=='finished' and r.get('test'));changed.pop(i);reject('drop-real-leaf-result',changed)
    changed=copy.deepcopy(rows);target=next(r['id'] for r in changed if r['kind']=='finished' and r.get('test'));changed=[r for r in changed if not(r.get('id')==target and r['kind'] in ['started','finished','dynamic_registered'])];reject('drop-entire-execution-keep-final-plan',changed)
    changed=copy.deepcopy(rows);i=next(i for i,r in enumerate(changed) if r['kind']=='finished' and r.get('test'));changed.insert(i,copy.deepcopy(changed[i]));reject('duplicate-source-result',changed)
    changed=copy.deepcopy(rows);next(r for r in changed if r['kind']=='finished' and r.get('test'))['status']='FAILED';reject('forged-green-summary-over-failed-case',changed)
    changed=copy.deepcopy(rows);changed[-1]['guard_denials']=1;reject('guarded-run-as-pass',changed)
    changed=copy.deepcopy(rows);changed[0]['jupiter_version']='5.9.1';reject('wrong-original-engine',changed)
    changed=copy.deepcopy(rows);changed[0]['jupiter_origin']='file:/D:/unknown.jar';reject('unbound-engine-class-origin',changed)
    changed=copy.deepcopy(rows);changed=[r for r in changed if r['kind']!='plan_finished'];reject('missing-real-plan-end',changed)
    changed=copy.deepcopy(rows);next(r for r in changed if r['kind']=='final_descriptor' and r.get('test'))['method']='unrelated';reject('wrong-original-source-method',changed)
    changed=copy.deepcopy(rows);changed=[r for r in changed if not(r['kind'] in ['started','finished'] and not r.get('test'))];reject('drop-all-container-lifecycles',changed)
    changed=copy.deepcopy(rows);target=next(r['id'] for r in changed if r['kind']=='finished' and not r.get('test'));changed=[r for r in changed if not(r.get('id')==target and r['kind'] in ['started','finished'])];reject('hide-failed-container-by-dropping-lifecycle-pair',changed)
    changed=copy.deepcopy(rows);target=next(r['id'] for r in changed if r['kind']=='finished' and not r.get('test'));
    for r in changed:
        if r.get('id')==target and r['kind'] in ['started','finished']:r['kind']='unrecognized-callback'
    reject('hide-container-in-unknown-event-kind',changed)
    changed=copy.deepcopy(rows);node=copy.deepcopy(next(r for r in changed if r['kind']=='discovered' and not r.get('test')));node['id']+='/'+'[container:foreign]';node['parent']=next(r['id'] for r in changed if r['kind']=='discovered' and r.get('parent') is None);node['kind']='started';end=copy.deepcopy(node);end['kind']='finished';end['status']='SUCCESSFUL';i=next(i for i,r in enumerate(changed) if r['kind']=='final_descriptor');changed[i:i]=[node,end];reject('extra-nontest-lifecycle-outside-original-plan',changed)
    return dict(count=len(cases),all_rejected=True,cases=cases)
