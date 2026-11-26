# Java combined-suite validation — 2026-11-12

Continue the unresolved combined-suite gate from ../../phase09-local-validation-plan.md.
This task owns only this evidence directory and tools/java-v1-golden changes if a reproducible defect requires them. The branch callback probe is being edited in another active chat and is left untouched.

Acceptance: execute ProtocolV1GoldenFrameTest, AbstractTCInboundHandlerExceptionTest, and SessionHolderRetryTimeoutRecoveryTest against Java cba73e180fd8896a99df6415b859d275122feaa7 in test-suite/test-new-version, without changing golden bytes or weakening round-trip equality. Record actual test counts, failures, source and overlay hashes. Do not infer remote CI success from a local run.

No new branch, tool installation, SDD initialization, commit or push.

Completed: selected combined suite 17/17 passed, Maven exit 0. Evidence: README.md, test-summary.json and original reports. Remote CI cause remains unresolved; Phase 0 remains open.
