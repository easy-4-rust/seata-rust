# Explicit Java V1 transaction-response exception profile

This is an opt-in compatibility candidate for the pinned Go source, implementing the existing Java-authority decisions. It does not renumber legacy Go errors or change the default codec registry. [Plan](../../docs/phase-0/go-java-v1-exception-profile-plan.md), [candidate patches](../../compatibility/seata-go/README.md).

```powershell
$env:PYTHONDONTWRITEBYTECODE = '1'
python tools/go-java-v1-exception-profile/run.py
python -m unittest discover -s tools/go-java-v1-exception-profile -p test_validation.py -v
```

The established local JDK17/Maven/Go/Rust environment, pinned source exports and caches are required. Files stay on D:/E:. No installation, specification initialization, branch change or live TC is performed. The `.go` files here are separate source inputs copied or compiled by explicit filename lists inside the pinned Go module; this folder is not itself a Go package.

The composed runner executes437 selected Java tests across15 fresh suites, including250 cases covering25 actual Java exception codes in ten transaction-response types. Twenty Java inputs are frozen before Maven; input/executed snapshots are separate. The new exception/length tests must match canonical executed bytes. Both Java test-resource JSON files are copied from frozen inputs and checked after Maven. Original source closure audits cover2122 Java production/resource/POM files and714 Go source/module files.

The same250 actual Java records feed these comparisons:

- Unmodified Go: exact98 divergent cases and152 matches.
- Go with only0001 length patch: exact60 divergent cases and190 matches.
- Go with0001+0002 and explicit profile API: all250 field/byte cases, pointer encodes and caller-unchanged checks must pass.

The parent runs codec/bytes/errors before0002 (99 top-level tests including the length regression); the candidate adds five profile tests for an exact104-function set. Unknown Java errors25..255, unmappable Go-only errors, truncated/trailing payloads, invalid lengths/UTF-8/result/status/lockable, nil/non-response inputs and lossy success text must fail explicitly. Payload size is capped at1MiB; individual text fields use valid UTF-8 byte lengths0..32767. Oversized input is rejected without truncation.

Additional regression checks cover all seven earlier Java failure samples with independently constructed Go semantics, the original14 transaction-response vectors, rejection of15 unsupported original non-response samples, and24 length-wire samples. The native29 payload/six-frame runner still checks unchanged default behavior. Rust's116-test standalone probe includes an independent typed250-case check and unknown-code rejection; Clippy and formatting are required. `SEATA_IT_EXCEPTION_PROFILE_DIR` points Rust to that same fresh capture.

Source snapshots include the consumed seven-case oracle, both patches and manifests, candidate/test/driver sources and existing regression inputs. Exact case identities—not only counts—are checked. Actual Go test files and applied candidate sources are hashed before/after execution. Success markers are published only after the complete source/fixture/test checks.

The resulting new entry point is `codec.NewJavaV1ResponseProfile(manager)`, with `Encode(value) ([]byte,error)` and `Decode(payload) (interface{},error)`. Supply an already initialized, stable native manager with0001 applied. Java-only semantic constants occupy separate Go domain values256..260; they require this profile, never a direct cast through the old default API. Java19 maps to `JavaV1NotRaftLeader`; Java20 maps to the existing Go `LockKeyConflictFailFast`. Go TCC/Fence/Saga-specific errors with no Java meaning are rejected.

Scope is serialized transaction-response payloads. This does not integrate the profile into Getty, select peer versions automatically, implement missing merges, establish global-state business behavior, fix upstream releases or deliver production Rust crates. The original default Go divergence remains recorded. Historical evidence retains its actual counts; current combined Java wrappers select433 tests on Linux and437 on Windows.

The current combined harness also includes38 merged-payload cases:475 selected Java tests/16 suites,21 overlays and117 Rust probe tests. Historical run counts remain unchanged. [Merge payload runner and required transport stage](../go-java-v1-merge-profile/README.md).
