# Contributing

## Design concept (short version)

- **Ease of use first.** Few names, few lines for common jobs; every
  capability is an opt-in Cargo feature or a plain-data `Options` field.
- **Options are plain data.** One `Options` struct, public fields, no
  builder methods. New capabilities become new fields.
- **Features gate capabilities, never flip behavior.** A format feature
  adds a format; a value-parser feature adds table format ids; nothing
  changes the meaning of an already-valid input.
- **Filenames decide override order, extensions decide parsers** (last
  claimer of an extension wins — built-in and custom formats alike).
- **The table stage is generic.** Format modules lower files to
  `[[key, value, format], …]` rows; `parse_table` interprets them. Name
  shared things `Table*`, not `Csv*`.
- **Provenance is a debug aid.** `trace()` labels carry no config
  semantics.
- **No dot-path getter.** `value["a"]["b"]["c"]` is already safe —
  missing keys chain as `Null` instead of panicking — so it fully
  covers the need, and a `.get("a.b.c")` API would just be a second way
  to do the same thing. node-config's `config.get('a.b.c')` only exists
  because of JS history: `config.a.b.c` used to throw on an undefined
  intermediate and `config?.a?.b?.c` didn't exist yet; Rust's `Index`
  gives us the safe form natively.

CLAUDE.md holds the complete, rigorous spec — read it before changing
behavior. README.md is the user-facing overview; keep it simple.

## Workflow (TDD, strictly in this order)

1. Spec the behavior in CLAUDE.md (and in simplified form in README.md).
2. Write the tests — fixture folder, integration test, doc tests —
   against the spec.
3. Implement until the tests pass.

## Test layout

- One fixture per scenario: `tests/fixtures/<case>/` with `config/`
  (the input), `expect.json` (the plain merged result) and
  `expect.debug.json` (the serialized trace: `$id`-tagged leaves with
  value + source + format). Error cases have `config/` only.
- Binary spreadsheet fixtures (`.xlsx`/`.ods`) are never hand-edited:
  `cargo run --manifest-path tools/gen-sheets/Cargo.toml` (a
  zero-dependency Rust tool) regenerates them all — plus the
  `xlsx-sheets` example workbook — byte-deterministically; CI checks
  the binaries match the generator.
- Option variants get their own case folder with copied config files —
  never share one config between variant expectations.
- CLI cases: `tests/cli/<case>/` with `args.txt` and one `result.*`
  file, compared byte-for-byte against the binary's stdout.
- Fixture-dependent tests are `#[cfg]`-gated on the features their
  fixtures need; watch feature implications (`inet` implies
  `ipv4`+`ipv6`+`cidr`, `macaddr` implies `macaddr8`, `datetime`
  implies `date`+`time`) when writing `not(feature = …)` gates.
- Shape validators live in crate-private `src/valid.rs` with exhaustive
  accept/reject unit tests next to them.
- Examples are standalone crates under `examples/<name>/` with a
  committed `output.log`; regenerate the log whenever the example
  changes.

## Commands

```sh
cargo test                            # default features
cargo test --all-features             # every format + value parser
cargo test --features cli             # builds the binary + CLI stdout cases
cargo test --no-default-features --features yaml   # single-format build must pass
cargo test --doc                      # doc tests only
(cd examples/00_simple && cargo run)   # standalone example crates
```

When touching feature-gated behavior, run the matrix: default,
`--all-features`, `--features cli`, each single format via
`--no-default-features`, and partial value-parser combinations
(`csv,inet`, `csv,datetime`, `csv,macaddr`, …). Everything must pass
with zero warnings.

## Releasing a new version

Publishing is automated: pushing a `v*` tag runs the full CI and, once it
passes, publishes to crates.io via Trusted Publishing (OIDC — there is no
token to manage).

1. Commit all your changes first — a clean working tree keeps the tag
   accurate.
2. Bump `version` in `Cargo.toml` (the crates.io package is `c4-config`;
   the lib/binary stay `c4`). `cargo publish` uses this version, so it
   must not already be published.
3. Tag it **with a `v` prefix** matching the version — `v0.1.1`, **not**
   `0.1.1`. Only `v*` tags trigger the release workflow (`on.push.tags:
   ["v*"]`).

   ```sh
   git tag v0.1.1
   ```

4. Push the commit and the tag. Pushing the tag kicks off CI; the
   `publish` job runs `cargo publish` only after fmt + clippy, the MSRV
   check, the whole test matrix and the examples all pass:

   ```sh
   git push && git push --tags
   ```

Notes:

- The README install is `cargo add c4-config` (unpinned), so no version
  string there needs updating.
- If the release raises the minimum toolchain, bump `rust-version` in
  `Cargo.toml` and the MSRV job in `.github/workflows/main.yml` together.
