# bendec2rs

An independent Rust implementation of the **Bendec 0.52.0 Rust source emitter**.
The repository, Cargo package, library API and CLI are all named
[`bendec2rs`](https://github.com/hpn777/bendec2rs). The Rust library and executable
need neither Node.js nor private schemas. This repository does not itself imply
a crates.io publication or a production cutover.

## Install and use

Rust 1.85 or newer is required. Use the library:

```toml
[dependencies]
bendec2rs = "0.1.0"
serde_json = "1"
```

Install the released CLI with `cargo install bendec2rs --version 0.1.0 --locked`.
From a source checkout instead:

```sh
cargo install --path . --locked
echo '[{"name":"OrderId","alias":"u64"}]' | bendec2rs
bendec2rs request.json --output new_messages.rs
```

The library exposes:

```toml
[dependencies]
bendec2rs = { git = "https://github.com/hpn777/bendec2rs", tag = "v0.1.0" }
serde_json = "1"
```

This alternative uses the tagged Git source rather than crates.io.

```rust
let types = serde_json::json!([
    {"name": "OrderId", "alias": "u64"},
    {"name": "Order", "fields": [{"name": "orderId", "type": "OrderId"}]}
]);
let source = bendec2rs::generate_string(types.as_array().unwrap(), &serde_json::json!({}))?;
# Ok::<(), String>(())
```

CLI input is a definition array or `{"types": [...], "options": {...}}`;
the `typeDefinitions` array envelope is also accepted. Callers must explicitly
assemble grouped definitions into an array. `--batch` accepts newline-delimited
request objects and returns newline-delimited JSON strings. Errors exit nonzero.
Output files must not exist, including symlinks; rendering finishes before opening
the output. No automatic rustfmt, publication or file replacement occurs.

## Scope and compatibility limits

The target is exact source bytes from the pinned upstream emitter, including its
formatting conventions. This is **not** a binary codec, a port of Bendec's
JavaScript runtime, or its Java/C++ generators. Matching upstream does not imply
generated Rust compiles for every schema: consumers need the appropriate
serde/bitflags dependencies and any applicable project traits.

The JSON API uses upstream option names. Function-valued `typeMapping` entries
become string templates (`"char[]": "[u8; {{ size }}]"`). `forEachType` may be a
declarative source template using `{{ generated }}` and `{{ name }}`; JavaScript
is never evaluated. Arbitrary JavaScript callbacks are not portable.

For optional native callbacks, implement `NativeHooks` and call
`generate_string_with_hooks(types, options, metadata, hooks)`. This supports
fallible per-type transformations and union-discriminant mapping. Opaque hook
metadata stays separate from validated emitter options; callback errors propagate.
The ordinary `generate_string` path avoids allocating callback contexts.
Consumer-specific feature gates, conversions and post-processing belong in
separate adapters, not this crate.

Identifier splitting handles ASCII and Latin deburring, but not all Lodash
Unicode/emoji rules. Non-finite/out-of-range numbers and malformed/null schema
edge cases are not guaranteed to match upstream. These are pre-1.0 compatibility
limits; consumers must independently validate generated-code and wire contracts.
The pre-release audit below is finite coverage, not proof of compatibility for
arbitrary valid JSON schemas.

### 0.1.0 pre-release compatibility report (2026-11-11)

**Compatibility audit passed for the tested scope; publication is separate.** The full oracle corpus
passes all 22 cases (1,013 definitions, 872,912 output bytes), and all 11 Rust
original integration tests plus four new regression tests pass on stable and Rust
1.85.0. Formatting and warning-denying Clippy checks pass. The additional
differential audit has 113 JSON cases: 66 exact-byte matches, 14 co-rejections,
21 acceptance differences and 12 byte differences. All exact-scope cases pass:
source bytes match, or, for `__proto__`, the upstream rejection diagnostic matches
verbatim apart from the CLI error prefix. Other co-rejections do not establish
matching error messages or semantics. Cases within documented compatibility limits
are observations, not parity passes.

The first audit exposed seven unexpected case failures, now fixed without
changing golden fixtures or exempting identifiers. Uppercase ordinals are consumed
as whole words before acronym splitting: `a1STb` becomes `a_1st_b`, and the full
196-field ASCII boundary matrix matches upstream. Inherited JavaScript default
mappings are also reproduced: type names `constructor`, `toString` and `valueOf`
alias `[object Object]`, `hasOwnProperty` aliases `false`, and `__proto__` rejects
with `typeMap[typeName] is not a function`. These surprising outputs are upstream
quirks, not valid-Rust guarantees. Explicit declarative mappings override inherited
defaults; per-type metadata does not override mapping precedence.

Sampled Latin accents/ligatures, ordinary acronyms/digits, straight/curly
apostrophes, and U+0300–U+036F combining marks match. That sampling is not a promise
for every Latin identifier. Greek/Cyrillic case boundaries, emoji/ZWJ/flags/skin
tones, and combining marks outside that range can differ. `camelCase` only adds
a serde annotation and does not bypass field-name splitting.

Malformed/null definitions and options may be rejected by only one emitter, or
accepted with different output (for example, null descriptions). Non-finite JSON
number results such as `1e400`, malformed radix strings, and radix conversions
outside the portable range are not equivalent. Neither successful generation nor
matching upstream implies numeric ranges fit the declared Rust type. Literal
`NaN`/`Infinity` tokens are not valid JSON. JavaScript function callbacks and
custom prototype inheritance cannot be transported in JSON: the audit records
their upstream-only behavior separately. Declarative Rust source templates are
intentional API alternatives, not exact JavaScript callback emulation.

Reproduce against the pinned oracle without updating fixtures:

```sh
cargo build --locked --release
BENDEC_RECEIPTS=target/release-audit node scripts/release-audit.cjs
```

The audit retains every input, both outputs/errors, diagnostics, input hashes and
the tested binary hash. It exits nonzero for unexpected differences in the
exact-byte cases. Its wall time includes subprocess startup/IPC and is not an
emitter-only performance measurement. No golden fixtures were regenerated.

## Testing and development oracle

```sh
cargo test --locked
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo build --locked --release --examples
npm ci --ignore-scripts
BENDEC_RECEIPTS=target/oracle node scripts/compatibility.cjs --synthetic-only
```

Rust-only golden tests contain portable synthetic fixtures. Node.js is used only
by development scripts, is excluded from the Cargo package, and is not a runtime
dependency. `package-lock.json` pins the development oracle to `bendec@0.52.0`.
`BENDEC_ORACLE_ROOT` may explicitly select an existing installed Bendec package
directory; its version is checked. CI checks stable and MSRV 1.85 and compares
synthetic cases byte-for-byte against the pinned oracle.

Full-schema coverage is opt-in and **requires** `BENDEC_SCHEMA_ROOT` pointing to
an existing directory of schema JSON files. Every `.json` file except
`schema.json` is included, without filtering definitions. Each file must contain
`typeDefinitions` as an array or grouped object. Missing/invalid schema inputs
fail rather than silently falling back to synthetic-only coverage:

```sh
BENDEC_SCHEMA_ROOT=/path/to/schemas BENDEC_RECEIPTS=target/full-oracle \
  node scripts/compatibility.cjs
```

Mismatches fail, retaining both outputs when `BENDEC_RECEIPTS` is set. Updating
golden fixtures is a separate, explicit maintainer operation, never part of CI:

```sh
node scripts/compatibility.cjs --synthetic-only --write-fixtures --fixtures-only
```

## Performance

```sh
python3 scripts/benchmark.py --synthetic-only --receipts target/benchmark
# Full coverage instead requires explicit schemas:
BENDEC_SCHEMA_ROOT=/path/to/schemas \
  python3 scripts/benchmark.py --receipts target/full-benchmark
```

The benchmark first requires exact-byte acceptance, then interleaves Rust and
JavaScript samples and retains receipts. Generation timings exclude compilation,
startup, filesystem reads and JSON parsing; process/harness wall time and build
time are reported separately. JavaScript diagnostics are counted but not printed
during timed generation; compatibility checks retain them. Results are workload-
and machine-specific, not a universal speedup guarantee or a full pipeline result.

## Attribution and license

MIT licensed; see `LICENSE` for this project's license. The implementation and
golden fixtures target and derive from the MIT-licensed Bendec 0.52.0 emitter by
its upstream contributors. The original upstream notice is preserved verbatim in
`LICENSE-BENDEC`, included in the Cargo package. Upstream:
[Bendec on npm](https://www.npmjs.com/package/bendec/v/0.52.0).
