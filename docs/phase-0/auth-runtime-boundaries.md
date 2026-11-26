# Native V1 authentication boundaries

Authority: Java commit `cba73e180fd8896a99df6415b859d275122feaa7`, `AbstractCheckAuthHandler`, `DefaultCheckAuthHandler`, `RegTmProcessor`, `RegRmProcessor`, `EnhancedServiceLoader`, and the [local live evidence](evidence/local-auth-registration/README.md).

The default provider's TM/RM methods return true. Enabling `server.enableCheckAuth` activates the provider hook, but does not give the default provider a credential policy. A deployment must supply and validate its own policy. Disabling the switch bypasses an extension derived from `AbstractCheckAuthHandler`. The test runner checks these separately using three TC processes.

The enhanced loader sorts definitions ascending and selects the last entry. The fixture's priority200 exceeds the default100. A regression test uses the actual loader and a temporary service descriptor; the initial priority0 failed that test and the first live denial profile. These are fixture failures, not evidence of an original TC authentication bypass defect.

V1 registration responses serialize the identified flag and version, omitting inherited ResultCode/error text. Clients must reject `identified=false` even if a decoded Java object's inherited ResultCode defaults to Success. Rejection proof also requires a same-channel transaction stimulus to close without allocating a transaction. A timeout alone is insufficient.

## Observed logging issue: JAVA-AUTH-EXTRADATA-001

The original registration processors log the full request at INFO after success, including `extraData`. The live run `run_20261112_161334_96952c7a/extension/tc-stdout.log` records the public fixture credential in successful TM and RM messages. Therefore a production authentication design must not assume credentials placed in this field stay out of logs. The fixture value is deliberately public and no production secret is used.

Status: **OPEN for production design**. No upstream logging code or original JAR was changed. A production solution needs an explicit credential transport and redaction policy plus log verification; the test extension is not that solution.

## Reconnect scope

The probe creates two original AT branches through one RM connection, closes it, authenticates a new RM connection with both cached resources, then services original-identity callbacks on that new connection. The runner first requires the TC's old-channel removal log. Querying Finished is bounded and may observe intermediate states because commit processing is asynchronous. The client never resends Commit/Rollback or re-registers a branch.

This is process-local, simulated resource behavior. It does not establish arbitrary reconnect backoff, RM crash recovery, durable callback deduplication, SQL/Undo Log compensation, TLS, replay resistance, tenant isolation or production credentials. Those remain formal implementation and acceptance work.
