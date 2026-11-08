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
| root `src/lib.rs` facade | Feature-gated re-exports for TM/RM/micro/DB proxy | **KEEP concept / REFACTOR API** | Review feature combinations and avoid exposing internal implementation types as stable public API |
| root `src/main.rs` | Prints build-success and feature summary | **REWRITE V2 / DEFER V1** | Placeholder binary, not a production TC entry point |

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

### 8.1 Additional TC/session correctness findings

| Finding | Source | Disposition |
|---|---|---|
| `DefaultSessionManager.update_global_session_status` has the `session.set_status(status)` mutation commented out, then persists the supplied session reference. | `rseata-core/src/session/defaults/default_session_manager.rs` | **REFACTOR / verify callers** — prove the caller mutates the session before invoking this method; otherwise status persistence may write the previous value. |
| `DefaultGlobalSession::is_timeout` uses elapsed milliseconds compared to `begin_time_millis * 1000`, not the stored `timeout_millis`. | `rseata-core/src/session/defaults/default_global_session.rs` | **REWRITE method** — use checked elapsed-time arithmetic against the configured timeout and add boundary tests. |
| `get_sessions_by_condition` currently returns all values; the condition is not applied. | `rseata-core/src/store/memery_transaction_store_manager.rs` | **REWRITE query** — implement all SessionCondition fields and indexes before a TC uses it. |
| Memory store uses HashMap/RwLock and is process-local. | `rseata-core/src/store/memery_transaction_store_manager.rs` | **KEEP test adapter only** — never treat as durable TC persistence. |

## 9. rseata-micro and error modules

| Module | Decision | Notes |
|---|---|---|
| Tonic interceptor / gRPC context propagation | **REFACTOR** | V1 propagation must carry TC-issued XID over Rust service boundaries; keep framework adapters outside transaction core |
| procedural macro | **KEEP idea / REFACTOR** | API must guarantee transaction completion on Rust error, panic and cancellation paths |
| `global_transaction` macro implementation | **REFACTOR / do not copy as-is** | The inspected macro computes `_is_result` but does not use it; generated code assumes Result-style `?` and `Ok/Err` handling. Verify return types, panic mapping, cancellation and commit/rollback errors with compile-fail and runtime tests |
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

## 11. Exact audited source manifest and final V1 reuse decision

The following paths exist in the pinned RSeata tree and are the concrete audit anchors. The classification is about architectural concepts and implementation disposition; it is not a blanket source-copy approval.

| Crate | Audited source paths | Final V1 disposition |
|---|---|---|
| `rseata-core` | `src/types.rs`; `src/branch/{mod.rs,branch_manager_inbound.rs,branch_manager_outbound.rs,branch_transaction.rs,undo_log.rs}`; `src/session/{global_session.rs,branch_session.rs,session_condition.rs,session_helper.rs,session_manager.rs,session_life_cycle.rs,session_lifecycle_listener.rs}`; `src/session/defaults/{default_global_session.rs,default_branch_session.rs,default_session_manager.rs}`; `src/store/{transaction_store_manager.rs,memery_transaction_store_manager.rs}`; `src/lock/*`; `src/resource/*`; `src/grpc_client/*` | **REFACTOR concepts only**; do not use its custom XID or status/branch wire mapping; session/lock store patterns are future TC research, not V1 client implementation |
| `rseata-proto` | `proto/rseata.proto`; `src/lib.rs`; `build.rs` | **REWRITE / exclude from V1 native protocol**; independent Tonic/Protobuf contract |
| `rseata-tm` | `src/transaction_manager.rs`; `src/lib.rs` | **REFACTOR concept**; compare scope/role/propagation against Go; do not reuse gRPC client as Java native transport |
| `rseata-rm` | `src/resource/impl_branch_manager_inbound.rs`; `impl_branch_manager_outbound.rs`; `impl_branch_transaction_registry.rs`; `impl_resource_registry.rs`; `src/config.rs` | **REFACTOR interfaces**; rewrite protocol adapter and status mapping; add durable idempotency and reconnect registration |
| `rseata-tc` | `src/coordinator/default_coordinator/*`; `src/coordinator/default_core_holder/*`; `src/coordinator/core/at_core/*`; `src/coordinator/core/xa_core/*`; `src/grpc_service/{tm_service.rs,rm_service.rs}`; `src/init.rs`; `src/web/*`; `src/audit_logger/*` | **DEFER V1; REFACTOR V2**; no Rust TC implementation in V1 |
| `rseata-db-proxy` | `src/sea_orm/at/connection_proxy/*`; `src/sea_orm/at/transaction_proxy/*`; `src/sea_orm/at/undo_log.rs`; `src/sea_orm/at/undo_log_postgres.rs`; `src/sea_orm/xa/connection_proxy/*`; `src/sea_orm/xa/transaction_proxy/*`; `src/table_primary_select.rs`; `src/diesel/mod.rs` | **REFACTOR / REWRITE critical transaction paths**; never copy SQL string substitution for Undo Log writes; MySQL AT is implemented against Java/Go semantics, not assumed from SeaORM adapter structure |
| `rseata-micro` | `src/lib.rs` | **KEEP concept / REFACTOR**; framework propagation adapter only; V1 XID is TC-issued |
| `rseata-error` | `src/lib.rs`; `crates/rseata-error-derive/src/lib.rs` | **KEEP derive idea / REFACTOR taxonomy**; separate protocol, transport, TC rejection, unknown outcome and local DB errors |
| `examples/example-proto` | `proto/rseata.proto`; `src/{client.rs,lazy.rs,lib.rs}` | **DEFER**; custom protobuf demo |
| `examples/order-service`, `examples/user-service` | `src/main.rs`, service modules and gRPC integration | **KEEP scenario ideas only**; rewrite examples against Seata-Rust V1 public APIs |
| root facade / binary | `src/lib.rs`; `src/main.rs` | **KEEP facade concept / DEFER binary**; root main is not a production TC |

### 11.1 No-copy rules

1. No RSeata message type, Protobuf enum, XID generator, or gRPC service is accepted as the Java V1 wire contract.
2. No RSeata session or lock implementation is accepted as production durable storage without a separate Java TC semantic audit, crash-recovery proof and concurrency tests.
3. No AT SQL/Undo Log path is accepted without comparing SQL parsing, before/after images, lock keys, Undo Log schema/serialization, dirty-data checks and transaction atomicity to pinned Java/Go behavior.
4. Trait names and module boundaries may inspire Rust API design, but every reused abstraction must pass the reuse gate in §10.
5. V1 implementation order remains Go TM/RM architecture + Java protocol/semantics. RSeata does not change that order.

### 11.2 Final classification summary

- **KEEP (concept/reference):** event hooks, trait separation, error derive concept, example scenarios, facade layering.
- **REFACTOR:** core types, TM scope/context, RM resource registry, branch handlers, database adapter boundaries, micro integration.
- **REWRITE:** native protocol adapters, explicit enum mappings, XID handling, critical Undo Log SQL paths, any custom gRPC/Protobuf code presented as Java V1 transport.
- **DEFER:** RSeata TC, durable session/lock/retry implementation, XA and other non-V1 modes, custom Protobuf examples and binaries.

This final disposition is a V1 reuse boundary, not a judgment that RSeata has no value. It is a useful independent Rust reference, but the compatibility contract and V1 execution sequence remain Java Seata 2.x + Seata-Go v2.
