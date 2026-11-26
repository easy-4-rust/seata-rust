set -eu
base=/run/seata-validation-20261120
d() { docker -H unix://$base/docker.sock "$@"; }
name=seata-wsl-undo-constraints-20261123
test "$(d inspect --format '{{.Id}}' "$name")" = 0bbea0ec511fae526ccecf90ba5c4859072a8ef06a19cc1fc516d48b1383e7a2
test "$(d inspect --format '{{index .Config.Labels "seata-wsl-owner"}}' "$name")" = seata-undo-constraints-validation-20261123
d stop "$name"
d rm "$name"
test -z "$(d ps -aq)"
engine_pid=$(sudo -n cat "$base/dockerd.pid")
test "$(sudo -n readlink /proc/$engine_pid/exe)" = /usr/bin/dockerd
sudo -n cat /proc/$engine_pid/cmdline | tr '\000' '\n' > /mnt/d/codex-builds/postcheck-engine-args.txt
grep -Fx -- '--data-root=/var/lib/seata-validation-20261120/data' /mnt/d/codex-builds/postcheck-engine-args.txt
grep -Fx -- '--host=unix:///run/seata-validation-20261120/docker.sock' /mnt/d/codex-builds/postcheck-engine-args.txt
sudo -n kill -TERM "$engine_pid"
for n in $(seq 1 15); do
    if ! sudo -n kill -0 "$engine_pid" 2>/dev/null; then
        echo 'Owned engine stopped'
        exit 0
    fi
    sleep 1
done
exit 1
