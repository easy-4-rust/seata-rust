set -eu
base=/run/seata-validation-20261120
fixture=/mnt/d/codex-builds/mysql-mtls-20261123
test ! -e "$fixture"
mkdir -p "$fixture/certs" "$fixture/private"
printf '%s\n' '[req]' 'distinguished_name=dn' '[dn]' > "$fixture/root.cnf"
for authority in ca other-ca; do
    openssl req -config "$fixture/root.cnf" -x509 -newkey rsa:2048 -nodes -sha256 -days 2 -subj "/CN=Seata owned $authority" -addext 'basicConstraints=critical,CA:TRUE' -addext 'keyUsage=critical,keyCertSign,cRLSign' -keyout "$fixture/private/$authority-key.pem" -out "$fixture/certs/$authority.pem" 2>>"$fixture/keygen.log"
done
for identity in server client untrusted-client; do
    if test "$identity" = server; then
        authority=ca
        printf '%s\n' 'basicConstraints=critical,CA:FALSE' 'keyUsage=critical,digitalSignature,keyEncipherment' 'extendedKeyUsage=serverAuth' 'subjectAltName=IP:127.0.0.1' > "$fixture/$identity.ext"
    else
        authority=ca
        if test "$identity" = untrusted-client; then authority=other-ca; fi
        printf '%s\n' 'basicConstraints=critical,CA:FALSE' 'keyUsage=critical,digitalSignature' 'extendedKeyUsage=clientAuth' > "$fixture/$identity.ext"
    fi
    openssl req -config "$fixture/root.cnf" -new -newkey rsa:2048 -nodes -sha256 -subj "/CN=seata-fixture-$identity" -keyout "$fixture/certs/$identity-key.pem" -out "$fixture/$identity.csr" 2>>"$fixture/keygen.log"
    openssl x509 -req -in "$fixture/$identity.csr" -CA "$fixture/certs/$authority.pem" -CAkey "$fixture/private/$authority-key.pem" -CAcreateserial -out "$fixture/certs/$identity.pem" -days 2 -sha256 -extfile "$fixture/$identity.ext" 2>>"$fixture/keygen.log"
done
openssl verify -CAfile "$fixture/certs/ca.pem" -verify_ip 127.0.0.1 "$fixture/certs/server.pem"
openssl verify -CAfile "$fixture/certs/ca.pem" -purpose sslclient "$fixture/certs/client.pem"
d() { docker -H unix://$base/docker.sock "$@"; }
sudo -n mkdir -p "$base"
if ! d info >/dev/null 2>&1; then
    sudo -n sh -c "nohup dockerd --host=unix://$base/docker.sock --pidfile=$base/dockerd.pid --exec-root=$base/exec --data-root=/var/lib/seata-validation-20261120/data --iptables=false --bridge=none --ip-forward=false --ip-masq=false --group docker > $base/daemon.log 2>&1 </dev/null &"
fi
for n in $(seq 1 30); do if d info >/dev/null 2>&1; then break; fi; sleep 1; done
name=seata-wsl-mtls-20261123
d run --pull=never --net=host -d --name "$name" --label seata-wsl-owner=seata-mtls-validation-20261123 -v "$fixture/certs:/fixture_tls:ro" -e MYSQL_ROOT_PASSWORD=seata-mysql-fixture-only -e MYSQL_ROOT_HOST=% -e MYSQL_DATABASE=undo_mtls sha256:0f1a0ccadda3a1734d1db557e203ec58621d56fa248525016e85fe4f631311ec --bind-address=127.0.0.1 --port=43317 --ssl-ca=/fixture_tls/ca.pem --ssl-cert=/fixture_tls/server.pem --ssl-key=/fixture_tls/server-key.pem --require-secure-transport=ON
for n in $(seq 1 60); do
    if d exec -e MYSQL_PWD=seata-mysql-fixture-only "$name" mysql --host=127.0.0.1 --port=43317 --user=root --ssl-mode=REQUIRED -N -B -e 'SELECT VERSION(),@@require_secure_transport' 2>/dev/null; then break; fi
    sleep 1
done
d exec -e MYSQL_PWD=seata-mysql-fixture-only "$name" mysql --host=127.0.0.1 --port=43317 --user=root --ssl-mode=REQUIRED -N -B -e "CREATE USER 'mtls_fixture'@'%' IDENTIFIED BY 'seata-mysql-fixture-only' REQUIRE X509; GRANT SELECT,INSERT,UPDATE,DELETE,TRIGGER ON undo_mtls.* TO 'mtls_fixture'@'%'; GRANT PROCESS ON *.* TO 'mtls_fixture'@'%'; CREATE TABLE undo_mtls.x509_work(id INT PRIMARY KEY,v INT NOT NULL) ENGINE=InnoDB; INSERT INTO undo_mtls.x509_work VALUES(1,10); SELECT user,ssl_type,plugin FROM mysql.user WHERE user='mtls_fixture';"
