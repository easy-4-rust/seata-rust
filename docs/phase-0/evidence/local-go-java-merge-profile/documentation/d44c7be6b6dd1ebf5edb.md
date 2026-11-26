# Explicit Java V1 ordinary/merged payload profile

[Plan](../../docs/phase-0/go-java-v1-merge-profile-plan.md), [candidate patches](../../compatibility/seata-go/README.md). This implements the payload stage of GO-V1-MERGE-001. The required Getty/frame integration stage remains open; default Go codec registration is unchanged.

```powershell
$env:PYTHONDONTWRITEBYTECODE = '1'
python tools/go-java-v1-merge-profile/run.py
python -m unittest discover -s tools/go-java-v1-merge-profile -p test_validation.py -v
```

Use the established cached Java/Go/Rust environment and D:/E: paths. No tools are installed, specification system initialized, branch changed, or live TC started. The `.go` files are distinct candidate/test/program inputs, copied or compiled by explicit filenames inside an owned Go cohort; this folder is not a standalone Go package.

The composed runner requires475 selected Java tests/16 fresh suites, Rust117 plus Clippy/formatting, and110 top-level Go tests in codec/bytes/errors. It inherits the complete0001/0002 verification, exact98/60/0 exception comparisons and prior seven failure semantics. Source bytes, XML reports, both Java test resources, all714 original Go members, applied source additions, executed tests/harnesses and the complete Go file set are checked. All three Rust fixture environment variables point to captures from the same fresh Java log, including nine control frames.

Thirty-eight new Java cases cover25 single-type containers, all25 with/without IDs, all12 result types, all250 error meanings, nested request/result containers, four signed timeout boundaries, two absent versions and null/Unknown report states. Six original Java merges must reproduce actual missing-codec failure in the reference. The explicit candidate must pass all44 merge cases,29 original ordinary payloads, typed/pointer/caller-preservation checks and Java semantic checks. Tests also cover malformed/truncated/trailing data, partial IDs, invalid types/counts/enums/UTF-8, requests inside results, cycles and symmetric depth/node budgets.

`codec.NewJavaV1MessageProfile(manager)` provides `Encode(value) ([]byte,error)` and `Decode(payload) (interface{},error)`. Supply a stable initialized native manager with0001/0002 applied. Supported payloads are the original25 native model types and59/60. Limits are8MiB payload,1024 children/container,2048 total nodes and depth8; they are candidate safeguards, not Java defaults. The decoder uses explicit field schemas to locate children, not a nonexistent universal child-length prefix. Results accept only the12 ordinary result families.

Go native timeout decoding is unsigned; this profile explicitly restores Java signed-int milliseconds. Encoding rejects sub-millisecond/out-of-i32 values rather than silently changing the Java timeout. Native casts, truncation and ignored fields are rejected by strict boundary parsing and reconstructed-field comparison. Only zero-length byte slices normalize nil/empty; strings, versions and nullable states do not get a generic default rule.

Registration response `Version` is **raw wire metadata**, not a direct Java getter. Use `profile.RegistrationVersion(response, javaCurrent)` for `JavaV1VersionInfo{WireVersion *string, JavaVersion string}`. Absent wire version remains nil; Java getter meaning requires explicit Java current-version context. The Java oracle shows that re-encoding its decoded default produces different bytes. Go/Rust preserve the raw absence and expose the getter semantics separately; no Java zero-version byte-roundtrip claim is made. Inherited registration ResultCode/Msg fields are not V1 result indicators: use the actual identified flag.

Use `profile.ReportStatus(request)` to obtain `(status,present,error)`; Java's255 sentinel is absent, while0 is a present Unknown state. The native field retains the wire value. These accessors work on direct values/pointers and children returned from merged messages.

The fixture set at `tests/fixtures/protocol/v1/merge_profile` is created once; later runs compare it. The Rust composition test reuses ordinary/exception payloads whose independent typed expectations remain part of the full suite, then checks containers/IDs/order, signed values and optional metadata. It does not replace those independent tests with a self-roundtrip assertion.

This is not a production Rust component, complete Go client, peer negotiation, compression/V2 or Getty delivery. Original59/60 codecs remain absent in the default registry. Next stage must explicitly integrate and verify the actual native frame read/write path with error propagation and default behavior regression; payload success does not close that stage or WP-00.
