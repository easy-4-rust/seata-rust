# Native V1 registration authentication and RM reconnect

This is a Phase0 test harness, not a production credential implementation. It uses the pinned, unchanged Java TC JAR and the standalone Rust probe. Development and runtime paths must remain on D:/E:; the default runtime directory deliberately contains spaces.

Run from the repository root with the existing JDK17, Maven, cached dependencies and exported pinned Java build available:

```powershell
$env:PYTHONDONTWRITEBYTECODE = '1'
python tools/java-tc-integration/auth_validation/run.py
```

The runner audits all tracked Java production sources/resources/POMs against commit `cba73e180fd8896a99df6415b859d275122feaa7` before and after fresh focused Java tests. Only its two test overlays are copied to the separate D: build. The original executable JAR is hash checked. An external class directory contains only the test policy and its service descriptor; the original registration processors, channel manager, codec, storage and TC application execute unchanged. `backend_recovery/run.py` supplies shared process, hashing and source-audit helpers; no backend recovery scenario is invoked.

Three fresh FILE TC processes on isolated loopback port28091 provide controls:

| Profile | Provider and switch | Required observation |
|---|---|---|
| default | Original default provider, checks enabled | Missing credential still identifies TM/RM: the original default is permissive |
| extension | External test provider, checks enabled | Six denials per role plus four resource denials for RM; valid credentials support two-resource reconnect |
| bypass | External test provider, checks disabled | Wrong credential identifies both roles, demonstrating the switch bypasses the extension |

The external checker accepts only the public fixture credential `phase0-local-only`, application `phase0-auth`, group `default_tx_group`, and RM resources `mysql://auth-a` and `mysql://auth-b`. `phase0-throw` injects an exception. Neither value is a secret. This does not establish credential distribution, TLS, replay protection or production authorization. The actual enhanced service loader sorts ascending and chooses the last definition, so the fixture priority must exceed the default provider's100.

Each denied registration must decode `identified=false`; transaction ResultCode is not present in this response body. On the same channel the probe sends exactly one GlobalBegin, then requires EOF/reset rather than timeout or transaction data. The runner also requires the fresh FILE global store to remain empty after all16 stimuli. This demonstrates functional rejection and absence of transaction allocation, not a direct inspection of every private channel map.

For reconnect, two branches on different resources are first registered through the old authenticated RM connection. A fresh RM connection re-registers the full cached resource list. An owned PID/port marker coordinates with the runner, which must observe the old RM channel removal in the actual TC log before releasing the client. Commit and rollback then require the original XID/BranchId/resource on the new connection, matching callback responses, TC completion logs and subsequent Finished status. No branch is registered a second time. Simulated callbacks do not prove SQL/Undo Log compensation or durable RM state.

Evidence is written to a new `docs/phase-0/evidence/local-auth-registration/run_*` directory; failed attempts are retained. Cleanup terminates only the TC/client processes started by this run and checks that the port is released. It preserves all runtime data and never resets another store. Freshness, source audit and Java tests must pass before live profiles begin.
