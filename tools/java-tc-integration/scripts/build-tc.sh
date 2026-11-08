#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/common.sh"

require_command mvn
mkdir -p "$TC_WORK_DIR"

if [[ ! -d "$TC_SOURCE_DIR/.git" ]]; then
  git clone --filter=blob:none --no-checkout https://github.com/apache/incubator-seata.git "$TC_SOURCE_DIR"
fi

git -C "$TC_SOURCE_DIR" fetch --depth 1 origin "$JAVA_SHA"
git -C "$TC_SOURCE_DIR" checkout --detach "$JAVA_SHA"

actual="$(git -C "$TC_SOURCE_DIR" rev-parse HEAD)"
[[ "$actual" == "$JAVA_SHA" ]] || {
  echo "Java Seata SHA mismatch: expected $JAVA_SHA, got $actual" >&2
  exit 1
}

(
  cd "$TC_SOURCE_DIR"
  built=false
  for attempt in 1 2 3; do
    echo "Building pinned Java TC (attempt $attempt/3)"
    if mvn -B -ntp -Prelease-seata-jar -pl server -am \
      -Dmaven.wagon.http.retryHandler.count=5 \
      -DskipTests \
      -Dcheckstyle.skip=true \
      -Dspotless.check.skip=true \
      clean package; then
      built=true
      break
    fi
    if [[ "$attempt" -lt 3 ]]; then
      echo "Maven build failed; retrying after transient dependency/network failure" >&2
      sleep 10
    fi
  done
  [[ "$built" == true ]] || exit 1
)

[[ -f "$TC_JAR" ]] || {
  echo "expected executable server jar not found: $TC_JAR (the pinned server POM declares the self-contained artifact under the release-seata-jar profile)" >&2
  find "$TC_SOURCE_DIR/server/target" -maxdepth 1 -type f -name '*.jar' -print >&2 || true
  exit 1
}

echo "Built Java Seata TC at $actual"
echo "Executable: $TC_JAR"
