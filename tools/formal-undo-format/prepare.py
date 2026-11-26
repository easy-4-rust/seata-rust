"""Fetch exact declared Java oracle libraries into D only; no CLI or plugin installation."""
import hashlib
import json
from pathlib import Path
import urllib.error
import urllib.request

DEST = Path('D:/codex-builds/seata-undo-java-dependencies')
ARTIFACTS = [
    ('com/fasterxml/jackson/core', 'jackson-annotations', '2.18.3'),
    ('com/fasterxml/jackson/core', 'jackson-core', '2.18.3'),
    ('com/fasterxml/jackson/core', 'jackson-databind', '2.18.3'),
    ('com/fasterxml/jackson/datatype', 'jackson-datatype-jsr310', '2.18.3'),
    ('mysql', 'mysql-connector-java', '5.1.42'),
    ('mysql', 'mysql-connector-java', '8.0.27'),
    ('com/github/ben-manes/caffeine', 'caffeine', '2.9.3'),
]


def get(url):
    with urllib.request.urlopen(url, timeout=60) as response:
        return response.read()


def run():
    DEST.mkdir(parents=True, exist_ok=True)
    records = []
    for group, artifact, version in ARTIFACTS:
        filename = artifact + '-' + version + '.jar'
        url = 'https://repo.maven.apache.org/maven2/' + group + '/' + artifact + '/' + version + '/' + filename
        # 官方旧包不一定发布SHA256；明确记录官方算法，同时对归档固定SHA256。
        algorithm = 'sha256'
        try:
            checksum = get(url + '.sha256').decode().split()[0]
        except urllib.error.HTTPError as error:
            if error.code != 404:
                raise
            algorithm = 'sha1'
            checksum = get(url + '.sha1').decode().split()[0]
        path = DEST / filename
        cached = Path('C:/Users/hnxyh/.m2/repository') / group / artifact / version / filename
        data = path.read_bytes() if path.exists() else cached.read_bytes() if cached.exists() else get(url)
        if hashlib.new(algorithm, data).hexdigest() != checksum:
            raise ValueError('Official artifact checksum differs: ' + filename)
        if not path.exists():
            path.write_bytes(data)
        record = dict(artifact=artifact, version=version, path=str(path), url=url,
            official_checksum_algorithm=algorithm, official_checksum=checksum,
            sha256=hashlib.sha256(data).hexdigest(), bytes=len(data))
        records.append(record)
        print(filename + ' verified ' + record['sha256'], flush=True)
    (DEST / 'manifest.json').write_text(json.dumps(records, indent=2) + '\n', encoding='utf-8')


if __name__ == '__main__':
    run()
