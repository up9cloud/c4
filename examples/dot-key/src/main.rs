//! Everything `dot_key` (default: on) does to keys — in env files and
//! both table layouts.
//!
//! Plain dots nest: a `db.host` key becomes `{db: {host: …}}`. A key
//! **segment** may also end in array suffixes — only `[]` / `[<int>]`
//! groups make an array, anything else stays a literal key:
//!
//! - `name[<int>]` addresses element `<int>`: rows may arrive in any
//!   order (`app.csv` lists `servers[1]` before `servers[0]`) and the
//!   same index deep-merges into one element (`.host` + `.port`).
//!   **Skipped indexes leave `null` gaps**: `slots[1]` + `slots[4]`
//!   give a five-element array with nulls at 0, 2 and 3 — which is why
//!   `App.slots` below is a `Vec<Option<Slot>>`.
//! - `name[]` **appends** one new element per occurrence: each
//!   `tags[]` row in `app.csv` pushes one tag; in the `monsters.csv` db
//!   grid the two `drops[]` columns push two strings per record. Note
//!   that two `servers[].host` rows would push two *separate* elements —
//!   building one object per element takes the `[<int>]` form.
//! - Suffixes **chain** for nested arrays, one level per group: the
//!   `grid[0][1]` / `grid[1][]` rows build a `Vec<Vec<u8>>` matrix.
//!
//! They also chain through the dotted path (`a[0].b[].c`) and apply to
//! env keys too. Two caveats at the end: the expansion happens **within
//! one parse**, so across *sources* arrays still replace like any
//! array; and with `dot_key: false` every bracket stays literal.
//!
//! Run inside this folder: `cd examples/dot-key && cargo run`
//! (expected output: `output.log` next to this file)

use c4::{Format, Loader, Options};

#[derive(Debug, serde::Deserialize)]
#[allow(dead_code)] // fields are shown via Debug
struct App {
    db: Db,
    servers: Vec<Server>,
    tags: Vec<String>,
    grid: Vec<Vec<u8>>,
    slots: Vec<Option<Slot>>,
}

#[derive(Debug, serde::Deserialize)]
#[allow(dead_code)]
struct Slot {
    name: String,
}

#[derive(Debug, serde::Deserialize)]
#[allow(dead_code)]
struct Db {
    host: String,
    port: u16,
}

#[derive(Debug, serde::Deserialize)]
#[allow(dead_code)]
struct Server {
    host: String,
    port: u16,
}

#[derive(Debug, serde::Deserialize)]
#[allow(dead_code)]
struct Monster {
    name: String,
    stats: Stats,
    drops: Vec<String>,
    weak: Vec<Weak>,
}

#[derive(Debug, serde::Deserialize)]
#[allow(dead_code)]
struct Stats {
    atk: u32,
}

#[derive(Debug, serde::Deserialize)]
#[allow(dead_code)]
struct Weak {
    elem: String,
    mult: f64,
}

fn main() -> Result<(), c4::Error> {
    // kv rows: dotted nesting, out-of-order [0]/[1] indexes, [] appends,
    // a [i][j] matrix, and slots[1]/slots[4] leaving null gaps
    let app: App = Loader::new(Options {
        sources: vec![(Format::Csv, "app.csv", "kv").into()],
        ..Options::default()
    })
    .load()?;
    println!("{app:#?}");

    // db grid: a dotted column, two `drops[]` columns appending per
    // record, and `weak[0].*` columns building one object per record
    let monsters: Vec<Monster> = Loader::new(Options {
        sources: vec![(Format::Csv, "monsters.csv", "db").into()],
        ..Options::default()
    })
    .load()?;
    for monster in &monsters {
        println!("{monster:?}");
    }

    // the suffixes work on env keys too — but appending happens within
    // one parse: a *second source* saying tags[]=green builds its own
    // one-element array, which replaces app.csv's (arrays never merge
    // across sources)
    let value: c4::Value = Loader::new(Options {
        sources: vec![
            (Format::Csv, "app.csv", "kv").into(),
            ("env", "tags[]=green").into(),
        ],
        ..Options::default()
    })
    .load()?;
    println!(
        "tags after a second source appends tags[]=green: {:?}",
        value["tags"]
    );

    // dot_key: false — dots and brackets are all literal
    let value: c4::Value = Loader::new(Options {
        sources: vec![("env", "a.b[0]=1").into()],
        dot_key: false,
        ..Options::default()
    })
    .load()?;
    println!("dot_key off: a.b[0] = {:?}", value["a.b[0]"]);
    Ok(())
}
