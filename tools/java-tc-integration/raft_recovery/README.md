# Live original-TC RAFT retry-timeout diagnostics

The runner starts three real original Java TC/JRaft applications per fresh cohort, using the pinned unchanged executable JAR, separate D:/E: stores and existing JDK17/Maven. No container image, package installation or production Rust component is required. The external helper only seeds through original session lifecycle replication, inspects state, requests real JRaft snapshots and dumps standard JDK exception recordings.

```powershell
$env:PYTHONDONTWRITEBYTECODE = '1'
python tools/java-tc-integration/raft_recovery/run.py --host 192.168.1.10
```

The host must be an IPv4 address actually assigned to this machine. The original TC refuses loopback as its advertised host; the runner verifies local binding before launching any process. Peers use ports28092/28094/28096; native TC ports are28091/28093/28095. Test controls bind only127.0.0.1 on28101–28103 and require the per-run ownership token. All ports must be free. Default D: runtime path contains spaces. Each JVM is bounded to two active processors and512MiB heap. The test does not join an existing cluster.

Before every run, compare2122 Java production source/resource/POM files against pin `cba73e180fd8896a99df6415b859d275122feaa7`, freshly compile/run the four helper guard/observer tests, audit sources again, and verify the original JAR hash. Copy only the helper class to the external loader directory. The exported Java test build is independent of the original source/JAR. Formatter divergence fails preflight rather than silently testing another source version.

Separate cohorts seed Begin1, CommitRetryTimeout16 and RollbackRetryTimeout17. Each has two original XIDs and one AT branch on the first XID; the second record is branch-empty. Original session methods submit actual JRaft tasks. Nothing writes a private session map, snapshot bytes or RocksDB record directly. The two timeout cohorts must stay distinct because an early thrown exception could otherwise hide a later status in the reload loop.

For each cohort:

1. Require a unique leader, all three replicas' matching records/locks, and applied indices caught up to the observed leader's committed index.
2. Snapshot every actual member, then stop only the owned leader process.
3. Require another node to lead in a later term, with the original committed index applied. Check original records, branch identity/type and independent lock occupancy on both remaining members.
4. Poll fresh, uniquely named JFR dumps within a bounded deadline. Timeout evidence requires an event after the current pre-stop UTC boundary, the exact exception class/message, `SessionHolder.reload` and the asynchronous `RaftStateMachine.onLeaderStart` stack. Preserve intermediate dumps; an old matching event or leader-start log is insufficient. A separately compiled JDK-only `ReadRaftExceptions` streams the binary JFR, filters the exact thrown class and retains all recorded frames. The standard `jfr print` default five-frame export loses the sixth-frame async caller and cannot establish this proof. The reader is not loaded into the TC.
5. Restart the old process from the same data directory, verify a new PID and actual session-snapshot loading, then require all replicas to catch up.
6. Stop the current leader again and repeat election/state/exception checks on the two survivors.

Begin controls are intentionally different: leader reload queues the old branch for rollback; its unserved simulated RM branch remains RollbackRetrying5 with its lock, while the empty transaction completes and disappears. Timeout records must retain16/17 and their original branch/lock. No original end request or BranchRegister is replayed by the runner.

After both elections and the old-node restart, also query the actual TC console and independently run the Rust `--query-raft-timeout` mode over native V1. It registers only TM, then queries each original XID exactly once. Its unit tests reject Finished as timeout retention and prohibit mutation traffic. Persisted branch resource, XID and lock key come from the actual branch object; the empty record's literal observer resource is not presented as a persisted branch field.

The driver records diagnostics, not a successful recovery assertion. A retained, queryable record can coexist with an asynchronous reload exception. The matrix's no-loss/operator semantics and lock handling must be assessed from the actual evidence. This increment does not run natural retry deadlines, operator retries, native RM compensation, durable replay, a network partition or snapshot corruption recovery.

Evidence goes to a new `docs/phase-0/evidence/local-raft-retry-timeout/run_*` directory. Failed attempts remain. Cleanup stops only saved owned process handles and preserves all data/logs/snapshots. Shared process/source-audit helpers come from `backend_recovery/run.py`; none of its backend scenarios execute here.

Pure evidence gates can be run independently:

```powershell
python -m unittest discover -s tools/java-tc-integration/raft_recovery -p test_checks.py -v
```
