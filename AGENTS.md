# AGENTS.md

## Project overview

`repass` is a personal, educational password-manager project written in Rust.
It is not currently intended to provide production-grade security or multi-user
server functionality.

The workspace currently contains:

- `repass_generator` — password generation, dictionaries and separators.
- `repass_storage` — records, tags and vault persistence.

Keep the project understandable and small. Do not introduce enterprise-scale
architecture, networking, accounts, synchronization or UI frameworks unless
the user explicitly requests them.

## Working rules

- Inspect the relevant code, tests and git diff before changing files.
- Preserve user changes. Do not reset, discard or overwrite unrelated work.
- Make focused changes; avoid unrelated cleanup and broad refactors.
- Prefer a simple implementation that matches the existing design over a new
  abstraction introduced speculatively.
- Explain significant architectural or format changes in the final response.
- Do not claim that the project is secure or production-ready.

## Architecture boundaries

- Keep password generation independent from storage.
- `repass_storage` should not depend on `repass_generator` unless there is a
  concrete domain-level reason. A future application or binary should compose
  the crates instead.
- Keep domain models separate from serialization and file-format details.
- Use stable, explicit identifiers for persisted records and tags; do not use
  vector positions as long-term persisted IDs.
- Prefer owned data (`String`, `Vec`) for values loaded from or saved to disk.
  Use borrowed data only where the lifetime and ownership are explicit.

## Persistence and data format

The vault format is part of the public behavior of the project, even if the
project is used by only one person.

- Include a magic value and format version in any persisted format.
- Do not change serialized field order, enum variants or framing without a
  migration or an explicit format break.
- Use unambiguous binary framing or a well-defined text encoding. Do not parse
  arbitrary encrypted bytes with line-oriented APIs.
- Keep a single canonical serialized schema where possible; use explicit
  conversions for views or in-memory representations.
- Prefer atomic writes (temporary file followed by rename) before implementing
  destructive save operations.
- Add round-trip tests for every persisted type and compatibility tests when
  the format changes.
- Treat loss or silent corruption of vault data as a higher priority than
  performance improvements.

Security is not the current primary goal, but never knowingly add misleading
security behavior. In particular, do not describe encryption as secure until
key derivation, nonce handling, authentication failures, error handling and
file recovery have been deliberately designed and tested.

## Rust style

- Run `cargo fmt` on changed Rust files.
- Prefer `Result<T, E>` over `panic!`, `unwrap()` or `expect()` in library and
  persistence APIs.
- Use meaningful error types instead of `Result<_, ()>`.
- Validate constructor arguments and return errors for invalid configuration.
- Keep public APIs small and document important invariants.
- Avoid unnecessary cloning, allocations and lifetime complexity until a
  measurement demonstrates a problem.
- Do not optimize for SoA layouts, string pools or custom indexes without
  benchmarks and a clear memory/query benefit.
- Be explicit about whether a length means bytes, Unicode scalar values or
  grapheme clusters. Never split UTF-8 strings at arbitrary byte offsets.

## Password generator rules

- Preserve the generator's length contract: if an API promises an exact target
  length, test that generated output always has that length.
- Handle dictionaries containing multi-character and non-ASCII entries.
- Trim line endings when loading file dictionaries, unless preserving them is
  explicitly intended.
- Avoid unbounded retry loops and panics for empty or invalid dictionaries.
- Keep separator validation consistent across separator implementations.
- Add property or table-driven tests for boundary lengths, empty input,
  multi-byte text and separators longer than one byte.

## Tags and records

- Maintain uniqueness invariants for tag IDs and names when constructing or
  loading `Tags`.
- Handle ID exhaustion and invalid IDs explicitly.
- Keep record and tag mutation operations consistent with their indexes.
- Do not expose unstable vector indexes as durable references.
- Define timestamp units and document them before using timestamps in the file
  format.

## Validation commands

For normal Rust changes, run:

```text
cargo fmt --check
cargo check --workspace --all-targets
cargo test --workspace
```

When changing serialization, also run focused round-trip and compatibility
tests. When changing generation, test exact lengths and representative
dictionaries.

If the repository does not currently compile, report the existing failures
separately from regressions introduced by the change. Do not hide unrelated
pre-existing failures by making broad changes.

## Change completion checklist

Before considering a change complete:

1. The implementation matches the requested scope.
2. Existing user changes remain intact.
3. Relevant tests were added or updated.
4. Formatting and checks were run, or failures are reported explicitly.
5. Persisted-data compatibility was considered for storage changes.
6. The final response briefly lists changed files, validation performed and any
   remaining limitations.
