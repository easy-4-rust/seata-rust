set -eu
base=/run/seata-validation-20261120
fixture=/mnt/d/codex-builds/mysql-tls-lifecycle-20261123
test ! -e "$fixture"
mkdir -p "$fixture/certs" "$fixture/private" "$fixture/issued"
printf '%s\n' '[req]' 'distinguished_name=dn' '[dn]' > "$fixture/root.cnf"
for authority in ca rotated-ca; do
    openssl req -config "$fixture/root.cnf" -x509 -newkey rsa:2048 -nodes -sha256 -days 2 -subj "/CN=Seata lifecycle $authority" -addext 'basicConstraints=critical,CA:TRUE' -addext 'keyUsage=critical,keyCertSign,cRLSign' -keyout "$fixture/private/$authority-key.pem" -out "$fixture/certs/$authority.pem" 2>>"$fixture/keygen.log"
done
for identity in server renewed rotated client; do
    authority=ca
    if test "$identity" = rotated; then authority=rotated-ca; fi
    if test "$identity" = client; then
        printf '%s\n' 'basicConstraints=critical,CA:FALSE' 'keyUsage=critical,digitalSignature' 'extendedKeyUsage=clientAuth' > "$fixture/$identity.ext"
    else
        printf '%s\n' 'basicConstraints=critical,CA:FALSE' 'keyUsage=critical,digitalSignature,keyEncipherment' 'extendedKeyUsage=serverAuth' 'subjectAltName=IP:127.0.0.1' > "$fixture/$identity.ext"
    fi
    openssl req -config "$fixture/root.cnf" -new -newkey rsa:2048 -nodes -sha256 -subj "/CN=seata-lifecycle-$identity" -keyout "$fixture/certs/$identity-key.pem" -out "$fixture/$identity.csr" 2>>"$fixture/keygen.log"
    openssl x509 -req -in "$fixture/$identity.csr" -CA "$fixture/certs/$authority.pem" -CAkey "$fixture/private/$authority-key.pem" -CAcreateserial -out "$fixture/certs/$identity.pem" -days 2 -sha256 -extfile "$fixture/$identity.ext" 2>>"$fixture/keygen.log"
done
touch "$fixture/index.txt"
printf '1000\n' > "$fixture/serial"
cat > "$fixture/sign.cnf" <<EOF
[ca]
default_ca=fixture
[fixture]
database=$fixture/index.txt
serial=$fixture/serial
new_certs_dir=$fixture/issued
private_key=$fixture/private/ca-key.pem
certificate=$fixture/certs/ca.pem
default_md=sha256
default_days=2
policy=policy
unique_subject=no
[policy]
commonName=supplied
EOF
for identity in server client; do
    openssl ca -batch -notext -config "$fixture/sign.cnf" -in "$fixture/$identity.csr" -out "$fixture/certs/$identity-expired.pem" -extfile "$fixture/$identity.ext" -startdate 20100101000000Z -enddate 20100102000000Z 2>>"$fixture/keygen.log"
    openssl ca -batch -notext -config "$fixture/sign.cnf" -in "$fixture/$identity.csr" -out "$fixture/certs/$identity-future.pem" -extfile "$fixture/$identity.ext" -startdate 20350101000000Z -enddate 20350102000000Z 2>>"$fixture/keygen.log"
done
cp "$fixture/certs/server.pem" "$fixture/certs/active-server.pem"
cp "$fixture/certs/server-key.pem" "$fixture/certs/active-server-key.pem"
cp "$fixture/certs/ca.pem" "$fixture/certs/active-ca.pem"
openssl verify -CAfile "$fixture/certs/ca.pem" -verify_ip 127.0.0.1 "$fixture/certs/server.pem"
for variant in server server-expired server-future client client-expired client-future renewed rotated; do
    echo "$variant"
    openssl x509 -in "$fixture/certs/$variant.pem" -noout -dates -serial
done
d() { docker -H unix://$base/docker.sock "$@"; }
sudo -n mkdir -p "$base"
if ! d info >/dev/null 2>&1; then
    sudo -n sh -c "nohup dockerd --host=unix://$base/docker.sock --pidfile=$base/dockerd.pid --exec-root=$base/exec --data-root=/var/lib/seata-validation-20261120/data --iptables=false --bridge=none --ip-forward=false --ip-masq=false --group docker > $base/daemon.log 2>&1 </dev/null &"
fi
for n in $(seq 1 30); do if d info >/dev/null 2>&1; then break; fi; sleep 1; done
name=seata-wsl-tls-lifecycle-20261123
d run --pull=never --net=host -d --name "$name" --label seata-wsl-owner=seata-tls-lifecycle-validation-20261123 -v "$fixture/certs:/fixture_tls:ro" -e MYSQL_ROOT_PASSWORD=seata-mysql-fixture-only -e MYSQL_ROOT_HOST=% -e MYSQL_DATABASE=undo_tls_lifecycle sha256:0f1a0ccadda3a1734d1db557e203ec58621d56fa248525016e85fe4f631311ec --bind-address=127.0.0.1 --port=43317 --ssl-ca=/fixture_tls/active-ca.pem --ssl-cert=/fixture_tls/active-server.pem --ssl-key=/fixture_tls/active-server-key.pem --require-secure-transport=ON
for n in $(seq 1 60); do
    if d exec -e MYSQL_PWD=seata-mysql-fixture-only "$name" mysql --host=127.0.0.1 --port=43317 --user=root --ssl-mode=REQUIRED -N -B -e 'SELECT VERSION(),@@require_secure_transport' 2>/dev/null; then break; fi
    sleep 1
done
d exec -e MYSQL_PWD=seata-mysql-fixture-only "$name" mysql --host=127.0.0.1 --port=43317 --user=root --ssl-mode=REQUIRED -N -B -e "CREATE USER 'mtls_fixture'@'%' IDENTIFIED BY 'seata-mysql-fixture-only' REQUIRE X509; GRANT SELECT,INSERT,UPDATE,DELETE,TRIGGER ON undo_tls_lifecycle.* TO 'mtls_fixture'@'%'; GRANT PROCESS ON *.* TO 'mtls_fixture'@'%'; CREATE TABLE undo_tls_lifecycle.lifecycle_work(id INT PRIMARY KEY,v INT NOT NULL) ENGINE=InnoDB; INSERT INTO undo_tls_lifecycle.lifecycle_work VALUES(1,10);"
