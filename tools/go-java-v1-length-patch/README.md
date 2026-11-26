# Java V1 error-length patch candidate

This runner validates a separately identified Go compatibility candidate for GO-V1-FAIL-001/002. It never patches the original Go export or changes the upstream commit, original29 payloads, six original frames or original failure diagnostics. [Plan and limits](../../docs/phase-0/go-java-v1-length-patch-plan.md).

```powershell
$env:PYTHONDONTWRITEBYTECODE = '1'
python tools/go-java-v1-length-patch/run.py
```

Use the existing cached local Java/Go toolchains and source exports recorded by the control-envelope runner. Development files stay on D:/E:. There are no tool installations, specification initialization, branch changes, servers or external messages. Each invocation creates a fresh D: source cohort and an E: evidence directory; failed attempts are retained.

The runner requires fresh Java437 selected tests, including24 new actual Java encode/decode cases. All20 overlay inputs are frozen before Maven, ROOT source drift is rejected and input/executed snapshots are separate. The new test must execute its canonical input bytes. Fifteen fresh XML reports must have the exact expected counts and no failures/errors/skips. Java capture records use escaped ASCII JSON so a Windows console code page cannot corrupt Chinese message metadata.

The same capture is supplied to unchanged native Go codecs first: exactly21 failed-response subtests must fail while three success controls pass. Only then does `git apply --check` validate the explicit patch before application in the owned source copy. The patched codec/bytes/errors packages run without a test-name filter, with all24 cases required to pass. The original29/six-frame harness is also executed in that cohort. All714 original Go source/module files are audited against the verified archive; only two named files may differ in the patch cohort. Original reference files must remain unchanged before/after execution.

The new fixture set is created once at `tests/fixtures/protocol/v1/error_lengths`; later runs compare rather than overwrite it. An independent Rust test checks all24 Java byte records against explicitly constructed typed expectations and rejects trailing bytes. The whole116-test probe suite, Clippy and formatting are required. Rust remains a standalone test probe, not a production protocol crate.

The [patch and manifest](../../compatibility/seata-go/82570d905919c1c66bb025a3a5979f25390de498/manifest.json) identify both file hashes and the patch hash. Keep that identity in any later evaluation. A green patched cohort does not make the unmodified upstream reference compatible. This candidate covers valid UTF-8 byte lengths0..32767 and observed ASCII truncation; it does not fix oversized Unicode/malformed decoding, the error-code19/20 split, missing merge codecs, arbitrary peer version selection or production client behavior. No new Linux/current-head CI run is implied.

The current combined selection includes250 exception-code Java cases. The original187/115/80 historical length-candidate evidence remains unchanged. [Exception profile runner](../go-java-v1-exception-profile/README.md) supplies the fresh matrix before the new Rust test and adds opt-in0002 validation; standalone execution uses already certified matrix fixtures for its separate scope.

The current combined harness also includes38 merged-payload cases:475 selected Java tests/16 suites,21 overlays and117 Rust probe tests. Historical run counts remain unchanged. [Merge payload runner and required transport stage](../go-java-v1-merge-profile/README.md).
