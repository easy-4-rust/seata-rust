# RSeata Reference Audit — Module-by-Module Keep / Refactor / Rewrite

> **Status: source-level disposition, not code migration approval.**
>
> RSeata is an independent third-party Rust project by `oulover`. It is not owned by this project and is not the user's implementation. Seata-Rust V1 is designed from the existing Seata-Rust architecture plan and the Seata-Go v2 execution model. RSeata is used only as a Rust reference sample.
>
> Audited source snapshot: `oulover/rseata` commit `c6062776c72a2490f62f8ebb25012c60884f62bd` (2026-03-10).

## 1. Disposition vocabulary

- **KEEP (concept/reference):** retain the idea, boundary or implementation pattern. This does not grant permission to copy code without license/NOTICE review and tests.
- **REFACTOR:** the abstraction is useful, but API, semantics, persistence, concurrency or error behavior must be redesigned before reuse.
- **REWRITE:** the current implementation/contract conflicts with V1's Java/Go compatibility requirement or lacks required correctness guarantees.
- **DEFER:** outside V1 scope; do not implement now.

## 2. Workspace-level disposition

| RSeata module | Observed responsibility | Decision | Seata-Rust action |
|---|---|---|---|
| `rseata-core` | XID/resource IDs, branch/global session, coordinator traits, locks, events, transaction store | **REFACTOR** | Use as a Rust abstraction reference; rebuild V1 client core around Go's TM/RM contracts |
| `rseata-proto` | Custom Protobuf service contract | **REWRITE for Java compatibility / KEEP as optional reference** | Do not use as native Seata protocol; optional Rust-native API can be reconsidered in V2 |
| `rseata-tc` | Tonic TC server, coordinator, session orchestration, metrics/audit | **DEFER V1; REFACTOR V2** | V1 uses Java TC; V2 may reuse patterns after complete correctness audit |
| `rseata-tm` | Rust TM transaction manager | **REFACTOR** | Compare role/propagation/timeout behavior against Seata-Go; implement V1 TM from Go contract |
| `rseata-rm` | Resource registry and branch manager | **REFACTOR** | Rebuild inbound/outbound contract and phase-2 behavior against Go and Java |
| `rseata-db-proxy` | SeaORM AT/XA proxy, Undo Log and SQL execution | **REFACTOR** | Keep adapter layering ideas; rewrite or validate transaction boundary, image and recovery semantics |
| `rseata-micro` | gRPC context propagation / integration facade | **REFACTOR** | Preserve propagation ideas; V1 needs Java-compatible XID propagation and Rust framework integrations |
| `rseata-error` | Error types and derive macro | **KEEP concept / REFACTOR API** | Use typed protocol, transport, transaction and database errors; don't couple to custom Tonic statuses |
| `examples/order-service` | Example order application | **KEEP as reference only** | Rebuild examples against Seata-Rust public APIs after V1 implementation |
| `examples/user-service` | Example service and gRPC integration | **KEEP as reference only** | Reuse scenario ideas, not transaction assumptions |
| `examples/example-proto` | Protobuf demo | **DEFER** | Not part of Java native V1 compatibility |
| root binaries / web | Example TC process and web/management scaffolding | **DEFER / REFACTOR V2** | V1 does not ship Rust TC |

## 3. rseata-core module details

| Submodule | Current source area | Decision | Reason / action |
|---|---|---|---|
| Branch IDs and types | `branch/mod.rs` | **REWRITE protocol mapping** | RSeata BranchType internal discriminants are 1..4; Java/Go wire values are AT=0, TCC=1, SAGA=2, XA=3. Never direct-cast. |
| Branch status | `branch/mod.rs` | **REWRITE protocol mapping; REFACTOR domain model** | RSeata uses a reduced/differently ordered status set. Java exposes 0..15; Go client model differs. |
| Branch manager inbound/outbound | `branch/branch_manager_*.rs` | **REFACTOR** | Preserve interface separation but align request fields, status values and error semantics to Go/Java. |
| Branch transaction trait | `branch/branch_transaction.rs` | **KEEP concept / REFACTOR** | Separate branch execution from transport; add durable idempotency and typed outcomes. |
| Undo Log traits/model | `branch/undo_log.rs` | **REFACTOR** | Reconcile schema, image model, serializer and dirty-data validation with Go/Java AT behavior. |
| XID / ResourceId / ClientId | `types.rs` | **KEEP wrappers / REFACTOR semantics** | Strong newtypes are useful. Never use RSeata's XID generator for Java TC; accept TC-issued XID unchanged. |
| GlobalStatus | `types.rs` | **KEEP domain concept / REWRITE wire adapter** | Internal 0..20 matches Java ordering, but must be independent from custom Protobuf numbering. |
| Coordinator traits | `coordinator/*` | **REFACTOR** | Useful separation; V1 client doesn't implement TC coordinator. |
| Session interfaces | `session/*` | **REFACTOR** | Compare branch ordering, locking, status persistence and lifecycle callbacks with Java TC. |
| Default GlobalSession | `session/defaults/default_global_session.rs` | **REFACTOR** | Current timeout calculation is incorrect/suspicious: elapsed milliseconds is compared with begin_time_millis * 1000 instead of timeout_millis. |
| Default BranchSession | `session/defaults/default_branch_session.rs` | **REFACTOR** | Verify all Java branch fields, XA metadata, resource identity and phase-2 idempotency. |
| Session manager | `session/defaults/default_session_manager.rs` | **REFACTOR** | Audit atomicity between session state, branch list and store updates. |
| SessionCondition | `session/session_condition.rs` | **REFACTOR** | Query semantics must match requested filters and indexes. |
| Session store trait | `store/transaction_store_manager.rs` | **KEEP concept / REFACTOR contract** | Good abstraction candidate for future Rust TC durable storage. |
| Memory store | `store/memery_transaction_store_manager.rs` | **KEEP for tests only** | HashMap/RwLock is not durable TC storage; observed condition query returns all sessions with TODO semantics. |
| Lock traits | `lock/*` | **REFACTOR** | Retain resource lock abstraction; verify isolation, lock key parsing, lock ownership and persistence. |
| Event publisher/handlers | `event/*` | **KEEP concept / REFACTOR** | Event hooks are useful, but transaction state changes must not depend on best-effort observer success. |
| Config | `config.rs` | **REFACTOR** | Separate V1 client config from V2 TC config and validate unsafe defaults. |
| gRPC clients/interceptors | `grpc_client/*`, `grpc_layer.rs` | **DEFER / REFACTOR** | Optional transport only; custom gRPC service is not Java Seata native protocol. |

## 4. rseata-proto disposition

| Component | Decision | Reason |
|---|---|---|
| `rseata.proto` package/service definitions | **REWRITE for V1** | Independent Protobuf/Tonic API, not Java native Seata remoting |
| BranchTypeProto | **REWRITE mapping** | proto 0..3, RSeata internal 1..4; conversion currently defaults unknown values to AT |
| GlobalStatusProto | **REWRITE mapping** | omits AsyncCommitting and shifts Committed and later values relative to Java |
| BranchStatusProto | **REWRITE mapping** | different enum ordering and incomplete status set |
| TransactionManagerService | **KEEP as optional API design reference** | Not used by V1 Java TC client |
| ResourceManagerService streaming registration | **KEEP as optional API design reference** | Not the native Java RM registration protocol |
| HealthCheckService | **KEEP concept** | Generic health service idea, no V1 protocol compatibility implication |

## 5. rseata-tm disposition

| Component | Decision | Required comparison |
|---|---|---|
| `transaction_manager.rs` | **REFACTOR** | Go Begin/Commit/Rollback/Status API, timeout and error handling |
| `rseata-core/grpc_client/tm_grpc_client.rs` | **DEFER / REFACTOR** | replace custom gRPC dependency with native Seata protocol provider for V1 |
| transaction scope / context | **REFACTOR** | Launcher vs Participant, nested propagation, async task scope, context cleanup |
| global transaction macro integration | **KEEP idea / REFACTOR** | ensure panic/error/early-return paths always finalize correctly |

## 6. rseata-rm disposition

| Component | Decision | Required comparison |
|---|---|---|
| resource registry | **REFACTOR** | Go resource registration, client ID, resource IDs, reconnect and unregister |
| branch register outbound | **REWRITE protocol adapter** | Go BranchRegister codec field order and Java TC response semantics |
| branch report outbound | **REWRITE protocol adapter** | map Java status codes explicitly |
| branch commit inbound | **REFACTOR** | mode handler, idempotency, duplicate request, typed retryability |
| branch rollback inbound | **REFACTOR** | compensation correctness and durable completion |
| resource instruction stream | **DEFER** | custom Tonic stream does not replace Java TCP phase-2 instruction dispatch |

## 7. rseata-db-proxy disposition

| Component | Decision | Required comparison |
|---|---|---|
| SeaORM AT ConnectionProxy | **REFACTOR** | Go ATConn execution, SQL parsing, transaction context and branch registration |
| SeaORM AT TransactionProxy | **REFACTOR** | local commit outcome stages, Undo Log atomicity, report behavior |
| AT Undo Log manager | **REWRITE critical SQL paths unless proven equivalent** | observed insert path builds SQL by replacing `?` and quoting JSON; this is not acceptable as a production parameterization strategy |
| PostgreSQL Undo Log | **REFACTOR / DEFER V1** | schema, JSON types, locking, SQL dialect and compensation behavior |
| SeaORM XA ConnectionProxy | **REFACTOR** | XA START/END/PREPARE/COMMIT/ROLLBACK lifecycle and connection retention |
| SeaORM XA TransactionProxy | **REWRITE or deep refactor** | prove XA branch XID mapping, recovery scan and uncertain outcome handling |
| Diesel module | **DEFER** | module/feature existence alone does not establish an adapter |
| table primary select helper | **KEEP only after audit** | verify composite primary keys and dialect quoting |

## 8. rseata-tc disposition

V1 does not run this module. It remains useful for future Rust TC research.

| Component | Decision | Reason |
|---|---|---|
| gRPC TM/RM services | **DEFER** | custom service, not Java protocol |
| DefaultCoordinator | **REFACTOR V2** | compare with Java DefaultCoordinator and Seata-Go client expectations |
| DefaultCoreHolder | **REFACTOR V2** | validate mode dispatch and state ownership |
| ATCore | **REFACTOR V2** | compare lock acquisition, branch registration, commit and rollback |
| XACore | **REFACTOR V2** | XA prepare/recovery semantics not yet proven |
| Session persistence | **REWRITE V2 production backend** | current inspected concrete store is in-memory; durable storage and recovery required |
| Lock manager | **REFACTOR V2** | validate atomic conflict detection, transaction ownership and restart behavior |
| retry / timeout | **REWRITE V2 scheduler** | derive full Java state machine and durable retry policy |
| audit / metrics | **KEEP concepts / REFACTOR** | useful separation, but not proof of correctness |

## 9. rseata-micro and error modules

| Module | Decision | Notes |
|---|---|---|
| Tonic interceptor / gRPC context propagation | **REFACTOR** | V1 propagation must carry TC-issued XID over Rust service boundaries; keep framework adapters outside transaction core |
| procedural macro | **KEEP idea / REFACTOR** | API must guarantee transaction completion on Rust error, panic and cancellation paths |
| error derive macro | **KEEP concept** | derive support is reusable if error taxonomy is rebuilt around protocol/transport/transaction/database categories |
| error enum | **REFACTOR** | distinguish definite rejection from unknown outcome and retryable from unretryable branch failure |

## 10. Reuse gate

No source file is copied into Seata-Rust until the reuse ledger contains:

- upstream repository and exact commit;
- source file and license/NOTICE review;
- intended target module;
- Java/Go semantic comparison;
- protocol compatibility result;
- unit/integration/failure-injection evidence;
- decision and reviewer.

### Final disposition

**RSeata contributes Rust design references; Seata-Go supplies the V1 implementation model; Java Seata defines the compatibility contract.**

Do not migrate RSeata wholesale. Do not reject useful Rust abstractions merely because the project is third-party. Evaluate each abstraction independently, and keep code reuse separate from architectural inspiration.
