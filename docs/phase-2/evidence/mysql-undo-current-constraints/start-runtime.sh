set -eu
base=/run/seata-validation-20261120
d() { docker -H unix://$base/docker.sock "$@"; }
sudo -n mkdir -p "$base"
if ! d info >/dev/null 2>&1; then
sudo -n sh -c "nohup dockerd --host=unix://$base/docker.sock --pidfile=$base/dockerd.pid --exec-root=$base/exec --data-root=/var/lib/seata-validation-20261120/data --iptables=false --bridge=none --ip-forward=false --ip-masq=false --group docker > $base/daemon.log 2>&1 </dev/null &"
fi
for n in $(seq 1 30); do if d info >/dev/null 2>&1; then break; fi; sleep 1; done
name=seata-wsl-undo-constraints-20261123
d run --pull=never --net=host -d --name "$name" --label seata-wsl-owner=seata-undo-constraints-validation-20261123 -e MYSQL_ROOT_PASSWORD=seata-mysql-fixture-only -e MYSQL_ROOT_HOST=% -e MYSQL_DATABASE=undo_current_constraints sha256:0f1a0ccadda3a1734d1db557e203ec58621d56fa248525016e85fe4f631311ec --bind-address=127.0.0.1 --port=43317
for n in $(seq 1 60); do if d exec -e MYSQL_PWD=seata-mysql-fixture-only "$name" mysql --host=127.0.0.1 --port=43317 --user=root -N -B -e 'SELECT VERSION()' 2>/dev/null; then exit 0; fi; sleep 1; done
exit 1