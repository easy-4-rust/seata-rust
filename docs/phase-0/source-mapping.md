# Phase 0 — Upstream Source Mapping

> **Status: Initial source map · commit-pinned**
>
> This document identifies the primary source entry points for the V1 design and the future Rust TC. It is not yet an exhaustive mapping of every upstream class or function. Paths below are pinned to the snapshots listed in the [Phase 0 index](README.md).

## 1. Repository snapshots

| Project | Commit | Pinned tree |
|---|---|---|
| Java Seata 2.x | `cba73e180fd8896a99df6415b859d275122feaa7` | [Source tree](https://github.com/apache/incubator-seata/tree/cba73e180fd8896a99df6415b859d275122feaa7) |
| Seata-Go v2 | `82570d905919c1c66bb025a3a5979f25390de498` | [Source tree](https://github.com/apache/incubator-seata-go/tree/82570d905919c1c66bb025a3a5979f25390de498) |
| RSeata | `c6062776c72a2490f62f8ebb25012c60884f62bd` | [Source tree](https://github.com/oulover/rseata/tree/c6062776c72a2490f62f8ebb25012c60884f62bd) |

## 2. Responsibility map: TC / TM / RM

| Responsibility | Java Seata 2.x | Seata-Go v2 | RSeata | Seata-Rust target |
|---|---|---|---|---|
| Global transaction API | `tm/.../DefaultGlobalTransaction.java`, `tm/.../GlobalTransactionContext.java` | `pkg/tm/transaction_executor.go`, `pkg/tm/global_transaction.go` | `rseata-tm/src/transaction_manager.rs` | `seata-tm` |
| TM remote calls | `tm/.../DefaultTransactionManager.java`, `core/.../TmNettyRemotingClient.java` | `pkg/tm/transaction/getty/*`, `pkg/tm/transaction/grpc/*` | `rseata-core/src/grpc_client/tm_grpc_client.rs` | `seata-protocol` + `seata-transport` + `seata-tm` |
| TC message dispatch | `server/.../DefaultCoordinator.java` | Not implemented as a separate Go TC; delegates to Java TC | `rseata-tc/src/grpc_service/{tm_service,rm_service}.rs` | V1: Java TC; V2: `seata-tc` |
| TC branch-type dispatch | `server/.../DefaultCore.java`, `server/.../AbstractCore.java` | Uses Java TC | `rseata-tc/src/coordinator/core/{at_core,xa_core}/*` | V1: compatible RM; V2: Rust mode handlers |
| Session lifecycle | `server/.../GlobalSession.java`, `server/.../BranchSession.java`, `server/.../SessionManager.java` | Uses Java TC | `rseata-core/src/session/*` | V2: `seata-tc-session` |
| Global lock | `server/.../AbstractLockManager.java`, `server/.../LockManager.java`, storage lock managers | Uses Java TC | `rseata-core/src/lock/*` | V1: Java TC; V2: `seata-lock` |
| Resource registry | `core/.../ResourceManager*.java`, server channel/session code | `pkg/rm/rm_api.go`, RM remoting and resource registry | `rseata-core/src/resource/*`, `rseata-rm/src/resource/*` | `seata-rm` |
| RM inbound phase 2 | `core/.../RmBranchCommitProcessor.java`, `RmBranchRollbackProcessor.java` | `pkg/rm/rm_api.go`, `pkg/datasource/sql/at_resource_manager.go` | `rseata-rm/src/resource/impl_branch_manager_inbound.rs` | `seata-rm` + mode-specific handler |
| RM outbound register/report | Java RM datasource and remoting clients | `pkg/rm/rm_api.go`, `pkg/protocol/codec/*` | `rseata-rm/src/resource/impl_branch_manager_outbound.rs` | `seata-rm` + `seata-protocol` |

### Primary pinned source links

**Java**
- [GlobalStatus.java](https://github.com/apache/incubator-seata/blob/cba73e180fd8896a99df6415b859d275122feaa7/common/src/main/java/org/apache/seata/core/model/GlobalStatus.java)
- [BranchStatus.java](https://github.com/apache/incubator-seata/blob/cba73e180fd8896a99df6415b859d275122feaa7/core/src/main/java/org/apache/seata/core/model/BranchStatus.java)
- [BranchType.java](https://github.com/apache/incubator-seata/blob/cba73e180fd8896a99df6415b859d275122feaa7/core/src/main/java/org/apache/seata/core/model/BranchType.java)
- [DefaultCoordinator.java](https://github.com/apache/incubator-seata/blob/cba73e180fd8896a99df6415b859d275122feaa7/server/src/main/java/org/apache/seata/server/coordinator/DefaultCoordinator.java)
- [DefaultCore.java](https://github.com/apache/incubator-seata/blob/cba73e180fd8896a99df6415b859d275122feaa7/server/src/main/java/org/apache/seata/server/coordinator/DefaultCore.java)
- [AbstractCore.java](https://github.com/apache/incubator-seata/blob/cba73e180fd8896a99df6415b859d275122feaa7/server/src/main/java/org/apache/seata/server/coordinator/AbstractCore.java)
- [GlobalSession.java](https://github.com/apache/incubator-seata/blob/cba73e180fd8896a99df6415b859d275122feaa7/server/src/main/java/org/apache/seata/server/session/GlobalSession.java)
- [DefaultTransactionManager.java](https://github.com/apache/incubator-seata/blob/cba73e180fd8896a99df6415b859d275122feaa7/tm/src/main/java/org/apache/seata/tm/DefaultTransactionManager.java)
- [XID.java](https://github.com/apache/incubator-seata/blob/cba73e180fd8896a99df6415b859d275122feaa7/common/src/main/java/org/apache/seata/common/XID.java)
- [AbstractUndoLogManager.java](https://github.com/apache/incubator-seata/blob/cba73e180fd8896a99df6415b859d275122feaa7/rm-datasource/src/main/java/org/apache/seata/rm/datasource/undo/AbstractUndoLogManager.java)

**Go**
- [TM transaction executor](https://github.com/apache/incubator-seata-go/blob/82570d905919c1c66bb025a3a5979f25390de498/pkg/tm/transaction_executor.go)
- [TM context](https://github.com/apache/incubator-seata-go/blob/82570d905919c1c66bb025a3a5979f25390de498/pkg/tm/context.go)
- [RM API](https://github.com/apache/incubator-seata-go/blob/82570d905919c1c66bb025a3a5979f25390de498/pkg/rm/rm_api.go)
- [Protocol message constants](https://github.com/apache/incubator-seata-go/blob/82570d905919c1c66bb025a3a5979f25390de498/pkg/protocol/message/constant.go)
- [Protocol codec registry](https://github.com/apache/incubator-seata-go/blob/82570d905919c1c66bb025a3a5979f25390de498/pkg/protocol/codec/codec.go)
- [Branch protocol model](https://github.com/apache/incubator-seata-go/blob/82570d905919c1c66bb025a3a5979f25390de498/pkg/protocol/branch/branch.go)
- [AT connection](https://github.com/apache/incubator-seata-go/blob/82570d905919c1c66bb025a3a5979f25390de498/pkg/datasource/sql/conn_at.go)
- [AT transaction](https://github.com/apache/incubator-seata-go/blob/82570d905919c1c66bb025a3a5979f25390de498/pkg/datasource/sql/tx_at.go)
- [XA connection](https://github.com/apache/incubator-seata-go/blob/82570d905919c1c66bb025a3a5979f25390de498/pkg/datasource/sql/conn_xa.go)

**RSeata**
- [Core types](https://github.com/oulover/rseata/blob/c6062776c72a2490f62f8ebb25012c60884f62bd/rseata-core/src/types.rs)
- [Branch model and status](https://github.com/oulover/rseata/blob/c6062776c72a2490f62f8ebb25012c60884f62bd/rseata-core/src/branch/mod.rs)
- [Global session](https://github.com/oulover/rseata/blob/c6062776c72a2490f62f8ebb25012c60884f62bd/rseata-core/src/session/defaults/default_global_session.rs)
- [Transaction store trait](https://github.com/oulover/rseata/blob/c6062776c72a2490f62f8ebb25012c60884f62bd/rseata-core/src/store/transaction_store_manager.rs)
- [Memory transaction store](https://github.com/oulover/rseata/blob/c6062776c72a2490f62f8ebb25012c60884f62bd/rseata-core/src/store/memery_transaction_store_manager.rs)
- [Custom gRPC protocol](https://github.com/oulover/rseata/blob/c6062776c72a2490f62f8ebb25012c60884f62bd/rseata-proto/proto/rseata.proto)
- [TC TM gRPC service](https://github.com/oulover/rseata/blob/c6062776c72a2490f62f8ebb25012c60884f62bd/rseata-tc/src/grpc_service/tm_service.rs)
- [TC RM gRPC service](https://github.com/oulover/rseata/blob/c6062776c72a2490f62f8ebb25012c60884f62bd/rseata-tc/src/grpc_service/rm_service.rs)
- [SeaORM AT Undo Log](https://github.com/oulover/rseata/blob/c6062776c72a2490f62f8ebb25012c60884f62bd/rseata-db-proxy/src/sea_orm/at/undo_log.rs)

## 3. AT execution source map

| Execution concern | Java | Go | RSeata | Seata-Rust owner |
|---|---|---|---|---|
| SQL interception / driver proxy | `rm-datasource/.../ConnectionProxy.java`, executor packages | `pkg/datasource/sql/conn_at.go`, `exec/at/*` | `rseata-db-proxy/src/sea_orm/at/*` | `seata-at` + database adapter |
| Before/after image | SQL executor and SQL undo log builders | `pkg/datasource/sql/exec/at/*`, `undo/builder/*` | AT transaction proxy and image models | `seata-at-image` |
| Undo log persistence | `rm-datasource/.../undo/AbstractUndoLogManager.java` | `pkg/datasource/sql/undo/*`, `hook/undo_log_hook.go` | `rseata-db-proxy/.../at/undo_log.rs` | `seata-at-undo` |
| Branch registration | RM outbound client and branch register protocol | `pkg/datasource/sql/at_resource_manager.go`, `pkg/rm/*` | TC/RM branch manager traits | `seata-rm` |
| Global lock check | TC LockManager / lock store | delegated to TC; SQL client sends lock keys | RSeata lock manager and AT core | V1: TC; V2: `seata-lock` |
| Phase 2 rollback | Java RM rollback processor and UndoLogManager | SQL RM and undo executor | RSeata SeaORM AT proxy (needs audit) | `seata-at-undo` |

## 4. XA execution source map

| Concern | Java | Go | RSeata | Seata-Rust owner |
|---|---|---|---|---|
| XA branch resource | `rm-datasource` XA datasource packages | `pkg/datasource/sql/xa/*` | `rseata-db-proxy/src/sea_orm/xa/*` | `seata-xa` |
| XA connection | Java XA connection proxy | `conn_xa.go`, driver-specific XA connection | SeaORM XA connection proxy | database XA adapter |
| XA transaction lifecycle | RM XA implementation and TC branch coordination | `tx_xa.go`, XA resource manager | XA transaction proxy | `seata-xa` |
| Recovery | Java TC retry/recovery + XA resource behavior | Java TC handles global coordination | Not established by current source inspection | V2 TC + XA recovery adapter |

## 5. RSeata audit findings to carry into implementation planning

| Finding | Source evidence | Classification | Required action |
|---|---|---|---|
| Custom XID generator differs from Java | `DefaultGlobalSession::new` builds `hex(transaction_id):transaction_service_group`; Java `XID.generateXID` builds `ip:port:tranId` | **Verified in source** | V1 must accept TC-issued XID; do not reuse RSeata generator |
| BranchType numeric values are inconsistent | RSeata internal enum is 1..4; Protobuf enum is 0..3; conversion maps 1..4 and defaults unknown to AT | **Verified in source** | Replace with explicit protocol mapping and reject unknown values |
| BranchStatus wire enum order differs | RSeata internal BranchStatus 1..12; proto enum uses different ordering starting at Normal=0; Java/Go define their own numeric contracts | **Verified in source** | No direct enum cast; explicit conversion tables + conformance tests |
| GlobalStatus proto omits/reorders statuses | Internal model mirrors Java 0..20; proto enum stops at Finished and assigns Committed=8 while Java/Go AsyncCommitting=8, Committed=9 | **Verified in source** | Explicit conversion; expand/compatibility version design |
| Memory store is not durable | `MemeryTransactionStoreManager` uses HashMap/RwLock | **Verified in source** | V1 does not use RSeata TC; V2 needs durable store |
| Condition query placeholder | `get_sessions_by_condition` returns all sessions with TODO comment | **Verified in source** | Do not reuse as production query behavior |
| Timeout calculation suspicious | `is_timeout` compares elapsed time against `begin_time_millis * 1000` rather than the stored `timeout_millis` | **Verified in source** | Fix and test before any reuse |
| Custom Tonic service is not Java native remoting | RSeata defines independent `TransactionManagerService` / `ResourceManagerService` | **Verified in source** | Keep as optional internal API only; do not label Java protocol-compatible |

## 6. Required next source-mapping pass

This first map covers V1 protocol, TM/RM, AT/XA, and high-risk RSeata code. Before Phase 0 exit, add function-level call graphs for:

- Java `DefaultCoordinator` request dispatch to `DefaultCore` and branch-mode handlers.
- Java `GlobalSession` lifecycle callbacks and every status-changing method.
- Java AT branch register, lock acquire, local commit, Undo Log flush, and rollback path.
- Seata-Go Gettty and gRPC TM/RM request flows, including registration/reconnect.
- Seata-Go AT image generation, Undo Log serialization and compensation executor.
- RSeata coordinator, session lifecycle, lock manager and all persistence paths.

Do not infer behavior from class names. Each call graph must point to the actual method and test that demonstrates the behavior.