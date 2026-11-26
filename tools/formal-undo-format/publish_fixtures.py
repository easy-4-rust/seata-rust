"""Publish immutable original-byte probe assets and correct checkout-vs-Git resource provenance."""
import hashlib
import json
from pathlib import Path

ROOT=Path(__file__).resolve().parents[2]

def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def read(path):return json.loads(path.read_text('utf-8'))
def need(value,message):
    if not value:raise ValueError(message)
def packet(path):
    for row in read(path/'catalog.json')['members']:need(sha(path/row['path'])==row['sha256'],'Archive differs')
    return dict(path=path.relative_to(ROOT).as_posix(),catalog_sha256=sha(path/'catalog.json'))

def run():
    inventory=ROOT/'docs/phase-2/evidence/undo-source-inventory/run_20261115_231536_dbc7bfc1'
    peer=ROOT/'docs/phase-2/evidence/undo-document-peer/run_20261115_232203_005ad917'
    inventory_binding=packet(inventory);peer_binding=packet(peer)
    assets=read(inventory/'source-assets.json')
    for row in assets:
        need(sha(ROOT/row['canonical_Git_target'])==row['git_blob_sha256'],'Pinned asset differs')
        need(sha(ROOT/row['checkout_copy_target'])==row['checkout_sha256'],'Historical checkout copy differs')
        a=(ROOT/row['canonical_Git_target']).read_bytes();b=(ROOT/row['checkout_copy_target']).read_bytes()
        need(a.replace(b'\r\n',b'\n')==b.replace(b'\r\n',b'\n'),'Asset difference is not line endings only')
        row['difference_verified']='CRLF_checkout_vs_LF_Git_only'
    report=dict(status='CORRECTION_checkout_bytes_are_NOT_Git_blob_bytes',inventory=inventory_binding,
        java_pin='cba73e180fd8896a99df6415b859d275122feaa7',resources=assets,
        previous_sealed_evidence_modified=False,source_test_parity_established=False,
        correction='Historical java_source_assets were byte copies of the Windows checkout; canonical java_git_assets now copy literal pinned Git blobs. Seven resource copies are not seven test cases.')
    (ROOT/'docs/phase-2/java-resource-provenance-correction.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
    probes=[]
    for source in sorted((peer/'inputs').iterdir()):
        if not source.name.startswith('probe-') and source.suffix!='.report':continue
        target=ROOT/'tests/fixtures/undo/java_source_probes'/source.name
        target.parent.mkdir(parents=True,exist_ok=True)
        if target.exists():need(target.read_bytes()==source.read_bytes(),'Existing probe asset differs')
        else:target.write_bytes(source.read_bytes())
        probes.append(dict(source=str(source.relative_to(ROOT)),target=str(target.relative_to(ROOT)),sha256=sha(target)))
    need(len(probes)==19,'Expected 17 documents and two reports')
    (ROOT/'docs/phase-2/undo-source-probe-assets.json').write_text(json.dumps(dict(status='SELECTED_SOURCE_VALUE_PROBES',
        producer=peer_binding,assets=probes,original_JUnit_tests_executed=False,source_case_parity_complete=False),ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
    print(json.dumps(dict(pinned_resources=len(assets),source_value_probe_documents=17,reports=2)))

if __name__=='__main__':run()
