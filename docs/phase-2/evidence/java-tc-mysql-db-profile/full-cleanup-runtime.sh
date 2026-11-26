set -eu
base=/run/seata-validation-20261120
d() { docker -H unix://$base/docker.sock "$@"; }
name=seata-wsl-java-tc-full-profile-20261124
test "$(d inspect --format '{{.Id}}' "$name")" = a94e35cfb47c3e67a735564982f867a2050be1fa72b511a14d02c8ed80c95ee3
test "$(d inspect --format '{{index .Config.Labels "seata-wsl-owner"}}' "$name")" = seata-java-tc-full-profile-validation-20261124
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
