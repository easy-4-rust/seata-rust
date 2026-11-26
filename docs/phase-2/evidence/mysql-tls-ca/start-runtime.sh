set -eu
base=/run/seata-validation-20261120
fixture=/mnt/d/codex-builds/mysql-tls-ca-20261123-2
test ! -e "$fixture"
mkdir -p "$fixture/certs" "$fixture/private"
printf '%s\n' '[req]' 'distinguished_name=dn' '[dn]' > "$fixture/root.cnf"
openssl req -config "$fixture/root.cnf" -x509 -newkey rsa:2048 -nodes -sha256 -days 2 -subj '/CN=Seata owned fixture CA' -addext 'basicConstraints=critical,CA:TRUE' -addext 'keyUsage=critical,keyCertSign,cRLSign' -keyout "$fixture/private/ca-key.pem" -out "$fixture/certs/ca.pem" 2>"$fixture/keygen.log"
openssl req -config "$fixture/root.cnf" -x509 -newkey rsa:2048 -nodes -sha256 -days 2 -subj '/CN=Other fixture CA' -addext 'basicConstraints=critical,CA:TRUE' -addext 'keyUsage=critical,keyCertSign,cRLSign' -keyout "$fixture/private/other-ca-key.pem" -out "$fixture/certs/other-ca.pem" 2>>"$fixture/keygen.log"
openssl req -config "$fixture/root.cnf" -new -newkey rsa:2048 -nodes -sha256 -subj '/CN=seata-owned-mysql' -keyout "$fixture/certs/server-key.pem" -out "$fixture/server.csr" 2>>"$fixture/keygen.log"
printf '%s\n' 'basicConstraints=critical,CA:FALSE' 'keyUsage=critical,digitalSignature,keyEncipherment' 'extendedKeyUsage=serverAuth' 'subjectAltName=IP:127.0.0.1' > "$fixture/server.ext"
openssl x509 -req -in "$fixture/server.csr" -CA "$fixture/certs/ca.pem" -CAkey "$fixture/private/ca-key.pem" -CAcreateserial -out "$fixture/certs/server.pem" -days 2 -sha256 -extfile "$fixture/server.ext" 2>>"$fixture/keygen.log"
openssl verify -CAfile "$fixture/certs/ca.pem" -verify_ip 127.0.0.1 "$fixture/certs/server.pem"
openssl x509 -in "$fixture/certs/server.pem" -noout -dates -ext subjectAltName
d() { docker -H unix://$base/docker.sock "$@"; }
sudo -n mkdir -p "$base"
if ! d info >/dev/null 2>&1; then
    sudo -n sh -c "nohup dockerd --host=unix://$base/docker.sock --pidfile=$base/dockerd.pid --exec-root=$base/exec --data-root=/var/lib/seata-validation-20261120/data --iptables=false --bridge=none --ip-forward=false --ip-masq=false --group docker > $base/daemon.log 2>&1 </dev/null &"
fi
for n in $(seq 1 30); do if d info >/dev/null 2>&1; then break; fi; sleep 1; done
name=seata-wsl-tls-ca-20261123
d run --pull=never --net=host -d --name "$name" --label seata-wsl-owner=seata-tls-ca-validation-20261123 -v "$fixture/certs:/fixture_tls:ro" -e MYSQL_ROOT_PASSWORD=seata-mysql-fixture-only -e MYSQL_ROOT_HOST=% -e MYSQL_DATABASE=undo_tls_ca sha256:0f1a0ccadda3a1734d1db557e203ec58621d56fa248525016e85fe4f631311ec --bind-address=127.0.0.1 --port=43317 --ssl-ca=/fixture_tls/ca.pem --ssl-cert=/fixture_tls/server.pem --ssl-key=/fixture_tls/server-key.pem --require-secure-transport=ON --tls-version=TLSv1.2,TLSv1.3
for n in $(seq 1 60); do
    if d exec -e MYSQL_PWD=seata-mysql-fixture-only "$name" mysql --host=127.0.0.1 --port=43317 --user=root --ssl-mode=REQUIRED -N -B -e 'SELECT VERSION(),@@require_secure_transport' 2>/dev/null; then exit 0; fi
    sleep 1
done
exit 1
