set -eu
base=/run/seata-validation-20261120
d() { docker -H unix://$base/docker.sock "$@"; }
name=seata-wsl-java-tc-db-profile-20261123
test "$(d inspect --format '{{.Id}}' "$name")" = 58568dc24e0ec3d54249b92ebb53390c4091ea94fb1b0e9daa22524e2bb827a6
test "$(d inspect --format '{{index .Config.Labels "seata-wsl-owner"}}' "$name")" = seata-java-tc-db-profile-validation-20261123
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
