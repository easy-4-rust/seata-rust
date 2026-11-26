set -eu
base=/run/seata-validation-20261120
d() { docker -H unix://$base/docker.sock "$@"; }
name=seata-wsl-binding-current-engine-20261123
test "$(d inspect --format '{{.Id}}' "$name")" = 293c750e12924ac8db71908288da761e54e76bd0547c0927542e2c9c8ca837c9
test "$(d inspect --format '{{index .Config.Labels "seata-wsl-owner"}}' "$name")" = seata-binding-current-engine-validation-20261123
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
