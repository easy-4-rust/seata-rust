# Actual Java handler response comparison

The Java matrix emits structured events **after** independently asserting real handler/processor output fields and serialized bytes. `collect.py` requires the exact79 case identities and a successful135-test run:78 serializable responses plus the separate unencodable null-report diagnostic. It checks independent finite-matrix fields and Java wire layout, then exports create-only `.bin`/`.expect` pairs and provenance. Synthetic events in collector unit tests are not runtime certification.

```text
python tools/java-handler-wires/collect.py --log <successful-java-log> --destination <new-output-directory>
python -m unittest discover -s tools/java-handler-wires -p test_collect.py -v
```

The output directory must be new, or contain the exact same artifacts. A different log hash requires a separate destination even when payloads are unchanged. Case names, schemas, data types, fields, payloads, missing/duplicate cases and conflicting existing artifacts are validated before writing; generation never repairs or overwrites different files.

`go_probe/main.go` runs inside the pinned unpatched Seata-Go module. It constructs typed Go expectations for each captured response, compares native Go output, then decodes Java bytes. Same-name Java fail-fast20 maps explicitly to native Go19 for comparison only; the actual Java bytes are never rewritten. The fixed codecs retain their observed defects.

Rust tests in `handler_runtime_tests.rs` read every binary and independent normalized field record, require all78 filenames and perform exact field/re-encode checks plus all truncated prefixes/trailing data. They default to the frozen repository capture; `SEATA_IT_HANDLER_WIRE_DIR` selects a fresh capture or an isolated negative-control copy.

On a Bash environment with existing compatible git/Maven/JDK/Python3/Go/Rust, this wrapper runs fresh Java capture, Go and Rust against **one fresh directory**:

```bash
bash tools/java-handler-wires/verify.sh
```

It installs no toolchain and uses the exact Java/Go snapshots. The inner Java entrypoint supports `SEATA_HANDLER_WIRE_DIR` for its export destination. The wrapper is syntax checked; this Windows increment executed the equivalent individual Java/collector/Go/Rust commands on D/E, not the fresh-clone wrapper itself. No remote workflow change/run is claimed.

The observed split is62 covered-field/byte-compatible responses,6 OPEN error-text-length cases,1 OPEN error-ordinal case and9 opaque Java global-status19 cases. The latter preserve raw bytes/numbers, while the pinned Go `message.GlobalStatus` constants end at15; this is a domain-evidence gap, not another byte mismatch. No full state-machine interoperability is inferred.

[Runtime evidence](../../docs/phase-0/evidence/local-handler-runtime-20261112/README.md); [known Go differences](../../docs/phase-0/known-go-v1-failure-divergences.md). Null-report encoding, live authenticated TC, response loss, storage recovery and formal production components remain separate gates.
