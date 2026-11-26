# Local live TC evidence — 2026-11-11

This evidence supplements `../local-phase09/`; it does not replace the previous failed build record.

- `build-summary.txt`: exact successful build arguments, exit status and reactor tail. Full log remains in the isolated Java checkout; its SHA-256 is recorded in `manifest.json`.
- `build-network-wait.txt`: thread trace from the interrupted default-repository build.
- `maven-central-settings.xml`: temporary dependency source configuration; no global settings were changed.
- `application.yml`: isolated FILE TC configuration, native port 28091, auth disabled.
- `lifecycle.txt`: real Rust client output, TC-issued XIDs, successful commit/rollback response statuses.
- `tc-stdout.log`, `tc-stderr.log`: corresponding Java process output, including authoritative Begin and terminal statuses.
- `live-result.json`: Java SHA, JAR/config SHA-256, process identity, probe exit and port-release result.
- `rust-tests.txt`, `rust-fmt.txt`, `rust-clippy.txt`, `rust-checks.json`: current local checks.
- `bootstrap-args-*`: first startup failure caused by passing a Spring option to Seata's application argument parser. The runner now uses a JVM property.
- `run-live.ps1`: reproduction using an already built pinned server JAR; writes runtime data under D: and evidence here under E:. A rerun replaces the current live logs, so copy this evidence directory first if preserving a separate run. The script stops its TC process in `finally`.

Run from PowerShell:

```powershell
& ./docs/phase-0/evidence/local-phase09-live/run-live.ps1
```

Two different no-branch XIDs passed Begin → Status(Begin) → Commit/Rollback → Status(Finished). Finished alone is not treated as success; the client first requires the matching successful operation response (Committed=9 / Rollbacked=11).

The Java source checkout includes the previously added test overlays, but no tracked upstream source modifications. This is a local result for the dirty Rust validation worktree based on `130ad634279cef10cd2998fc3bfae82996224d66`, not remote CI evidence.

Not covered: branch registration/callbacks, AT database/undo behavior, authentication, restart recovery, graceful shutdown or the unresolved full Java suite. Windows Stop-Process is forced termination; only process exit and port release are asserted.
