//! DIY hot-reload: c4 stays a synchronous loader on purpose — watching
//! is policy-heavy (debounce, error handling, threading), so instead of
//! a `watch` feature this example shows the whole pattern: watch the
//! folder with `notify`, re-run `loader.load()` when something changes.
//!
//! The demo copies its config into a scratch folder, edits it twice,
//! prints each reload and exits, so the committed `output.log` is
//! reproducible. A real service would watch its actual config folder
//! and loop forever.
//!
//! Run inside this folder: `cd examples/watch && cargo run`
//! (expected output: `output.log` next to this file)

use std::fs;
use std::sync::mpsc;
use std::time::Duration;

use notify::{RecursiveMode, Watcher};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // a scratch copy so the demo can edit files freely
    let dir = std::env::temp_dir().join(format!("c4-watch-example-{}", std::process::id()));
    fs::create_dir_all(&dir)?;
    fs::copy("config/app.yml", dir.join("app.yml"))?;

    let loader = c4::Loader::new(c4::Options {
        sources: vec![dir.as_path().into()],
        ..c4::Options::default()
    });
    let mut current: c4::Value = loader.load()?;
    print_state("loaded", &current);

    // any notify event just means "something changed — reload and see"
    let (tx, rx) = mpsc::channel();
    let mut watcher = notify::recommended_watcher(move |event| {
        let _ = tx.send(event);
    })?;
    watcher.watch(&dir, RecursiveMode::Recursive)?;

    // scripted edits standing in for a human changing the config
    let edits = [
        "port: 9090\ngreeting: hello\n",
        "port: 9090\ngreeting: hi there\n",
    ];
    for edit in edits {
        fs::write(dir.join("app.yml"), edit)?;
        loop {
            // wait for a change (the event details don't matter — any
            // event is just a signal to reload; only a timeout is fatal)
            let _ = rx.recv_timeout(Duration::from_secs(10))?;
            // ...then debounce the burst a single save produces
            while rx.recv_timeout(Duration::from_millis(150)).is_ok() {}

            let reloaded: c4::Value = loader.load()?;
            if reloaded != current {
                current = reloaded;
                print_state("reloaded", &current);
                break;
            }
            // spurious event (metadata only) — keep waiting
        }
    }

    fs::remove_dir_all(&dir).ok();
    println!("done after {} changes", edits.len());
    Ok(())
}

fn print_state(label: &str, value: &c4::Value) {
    println!(
        "{label}: port = {:?}, greeting = {:?}",
        value["port"].as_u64(),
        value["greeting"].as_str()
    );
}
