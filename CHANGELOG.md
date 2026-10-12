# Changelog

Versions before 0.6.0 have no entries here; see the git tags.

## 0.6.0 — unreleased

### Upgrading from 0.5

Breaking API changes:

- **MSRV is 1.88** (was 1.85).
- **`Loader::trace()` returns a `Trace`** — the merged tree plus the
  warnings — instead of the bare `TracedValue`:

  ```rust
  let traced = loader.trace()?;          // 0.5: TracedValue
  let traced = loader.trace()?.value;    // 0.6: TracedValue; .warnings beside it
  ```

  CLI `--trace -f json` output changes shape the same way:
  `{"value": …, "warnings": […]}`.
- **`Error::Io` carries the path**: `Error::Io(e)` → `Error::Io { path,
  source }`. Its message now starts with the path, and
  `std::error::Error::source()` returns the I/O error.
- **`Value::Ipv4` / `Value::Ipv6` hold canonical text** (`String`) instead of
  `Ipv4Addr` / `Ipv6Addr`, like every other typed value, so `as_str()`
  works on them. For the address use the new `as_ip()`:

  ```rust
  Value::Ipv4(addr) => addr                    // 0.5: Ipv4Addr
  value.as_ip()                                // 0.6: Option<IpAddr>
  ```
- **`Error`, `Format`, `Value` and `SourceRef` are `#[non_exhaustive]`**: a
  `match` on them needs a `_` arm. New variants will no longer be
  breaking.

Behavior changes to check:

- **`load::<Value>()` keeps typed values.** It used to turn `DateTime`,
  `Ipv4`, `Uuid`, … into plain `Value::String`; now they arrive as
  `trace()` shows them, also in `Value` fields of your structs. `as_str()`
  still returns their text.
- **A `(value,)` override's `None` fields override nothing.** An unset
  `Option` (a CLI flag not given) used to erase the earlier value with
  null; an explicit null still overrides.
- **env**: in an unquoted value, ` #` starts a comment (`PORT=8080 # web`
  is `8080`); a quoted value ends at its closing quote; double quotes
  decode `\n \r \t \" \\`.
- **yaml**: `<<` merge keys are applied (a literal `<<` key no longer
  appears). yaml is read with `yaml_serde`, which replaces the deprecated
  `serde_yaml`.
- **A leading UTF-8 BOM is ignored** by every text format; env and ini
  used to keep it in the first key.
- **dot_key array indexes are limited**: one `name[<int>]` may skip at most
  10000 unset slots past the array's end, and no index may exceed
  1000000. Larger ones are an error instead of a huge allocation.
- **Errors read differently**: deserialize errors start with the key path
  (`db.port: …`); a missing or unreadable spreadsheet file is
  `Error::Io` instead of `Error::Parse`; "no active format claims this file's
  extension" names the feature or `formats` entry that fixes it.
- **CLI**:
  - a boolean flag given a value (`--dot-key=false`) is an error — it used
    to turn the option *on*;
  - ini output writes root keys, then one `[section]` per top-level
    object, and errors on what ini cannot hold;
  - env output quotes and escapes so c4 reads it back, and errors on keys
    env cannot hold;
  - csv output writes null as `key,null,json` (it used to write a `null`
    type id that c4 could not read back);
  - toml output omits null keys and names the key of a null inside an
    array;
  - warnings go to stderr as `c4: warning: …`.
- **Dependencies**: `yaml_serde` replaces `serde_yaml`; `toml` 1,
  `calamine` 0.36, `jsonc-parser` 0.34.

### Added

- **`cnf` format** (feature `cnf`, `.cnf`): reads `my.cnf`, `openssl.cnf` and
  the like as they are, following MySQL where dialects disagree — bare
  flags (`skip-networking`) are `true`, include directives are skipped
  with a warning.
- **Warnings** (`Trace.warnings`, `Warning`) for input c4 accepted but did
  not act on.
- **Typed loading of strings and numbers**: a string fills a `bool` or
  number field with the table type-id rules (`"8080"` → `u16`, `"yes"` →
  `bool`), so ini, env and cnf load into typed structs; a number or bool
  fills a `String` field with its text.
- **`Value::into_typed::<T>()`** to type one part of a loaded value:
  `config["db"].clone().into_typed::<Db>()`.
- **`Value` conveniences**: `From` for strings, bools, integers, floats,
  `Option`, `Vec` and `BTreeMap`; comparisons with plain values
  (`value["port"] == 8080`, `value["name"] == "c4"`); a compact-JSON
  `Display`; `as_ip()`.
- **TOML 1.1** syntax (`toml` 1.x): inline tables across lines with a
  trailing comma, the `\e` escape.
- `From<TracedValue> for Value`.

### Performance

Measured on a release build: a 21 MB jsonc file loads in 326 ms (was
602 ms); 5000 small files 20 ms (27 ms); 200 overriding files 220 ms
(284 ms); a 200k-row csv record grid as `Value` 236 ms (362 ms); a
100k-row kv csv 40 ms (64 ms).
