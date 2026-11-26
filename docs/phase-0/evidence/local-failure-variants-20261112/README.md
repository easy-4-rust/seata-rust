# Supplemental failure variants — 2026-11-12

This increment adds evidence for six previously uncovered transaction failure families plus a same-name fail-fast error-code comparison. It preserves the original29 canonical payloads/6 frames and does not modify upstream codecs or deliver production Rust components.

**Result:** four added payload pairs are byte-compatible; **three remain OPEN incompatibilities**. The diagnostic runner succeeds only if it observes the expected native bytes/semantics and classifies the differences correctly. Its green result is not an interoperability pass for those three cases. See [known issues and exact impact](../../known-go-v1-failure-divergences.md).

| Verification | Actual result |
|---|---|
| Unchanged pinned Go baseline | 29 payloads + 6 frames passed |
| New native Go failures | 7 independently typed decode/re-encode checks passed; 4 compatible / 3 OPEN differences |
| Initial Go/Java equality probe | Failed as expected for BranchReport/GlobalLockQuery failure error-text lengths; raw failing source/log retained |
| Pinned Java focused failures | 7/7 passed, including exact native Java encoding, independent object fields and Go rejection/misinterpretation |
| Pinned Java combined target | **56/56**, zero failures/errors/skips; 91-module BUILD SUCCESS in 01:57 |
| Rust complete suite | **81/81**, zero failed/ignored; 8 new tests, including full failure fields, Go difference behavior, every added Java truncation and trailing bytes |
| Exporter | 5/5 tests passed after observed missing-exporter RED; create-only/preflight, unsafe-name/collision, prefix/classification, CRLF/LF and repeatability checks |
| Static checks | Rust fmt/Clippy with warnings denied, Go vet and both shell syntax checks exit 0 |

Java upstream serializer-module39 tests are not double-counted into the selected target56. Original29 + added7 are 36 Java/Rust sample payloads; only the original29 + four added compatible cases establish 33 Go/Java sample agreements. Additional optionality and malformed-input tests are distinct from those payload counts.

## Provenance

- Rust worktree base: `130ad634279cef10cd2998fc3bfae82996224d66`.
- Java: `cba73e180fd8896a99df6415b859d275122feaa7`, controlled exact source snapshot on D.
- Go: `82570d905919c1c66bb025a3a5979f25390de498`, downloaded via commit-pinned GitHub archive after the GitHub connector transport failed. The archive's SHA-256 and relevant original source/module file equality are recorded in [source provenance](go-source-provenance.json); no upstream codec was patched.
- Go toolchain: existing go1.25.3 windows/amd64 with `GOTOOLCHAIN=local`, build/module/temp directories on D. No toolchain installation/upgrade.
- Tested Go harness and JSON are byte-identical to repository files. Maven formats Java whitespace; exact executed Java test source and lexical equality to overlays are recorded. Four source/build-resource copies of the new JSON are byte-identical.

[Execution metadata](execution.json) records counts, commands, hashes and open issue IDs. [Source hashes](source-sha256.json), all selected target JUnit XML reports, raw native/runtime logs, the independent initial failing equality probe and exported fixture manifest are retained here. The 14 binaries and reproducible exporter live at `tests/fixtures/protocol/v1/failure_variants/` and `tools/go-failure-variants/`.

## Boundaries

GO-V1-FAIL-001/002 concern type14/22 failed error-message lengths (Go8 vs Java16). Java throws BufferUnderflowException for the recorded Go bytes and Rust rejects them. GO-V1-FAIL-003 concerns Go fail-fast code19 vs Java20: the Go payload remains syntactically valid to Java/Rust but means NotRaftLeader, proving that parsing alone cannot establish compatibility.

No upstream fix or inferred dialect fallback was applied. The full Go compatibility objective remains in scope and these three issues stay open pending a separately reviewed fix/adapter and relevant evidence. Long error strings, further ordinal differences and arbitrary variants are not established by the seven cases. Java handler-exception/auth matrices, response loss/recovery and formal production phases remain separate unfinished work.

The modified shell entrypoints passed syntax checks; equivalent local Go/Maven invocations ran, while their fresh-clone shell wrappers were not themselves executed on Windows. No new live TC, database transaction, remote workflow, branch change, commit, push, installation or SDD initialization occurred.
