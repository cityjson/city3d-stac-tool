use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

/// A minimal but valid Item: the collection builder aggregates the spatial and
/// temporal extents from the items, so `bbox` and `datetime` must be real.
fn item_json(id: &str, x: f64) -> String {
    format!(
        r#"{{"type":"Feature","stac_version":"1.1.0","id":"{id}",
             "geometry":{{"type":"Point","coordinates":[{x},52.0]}},
             "bbox":[{x},52.0,0.0,{x},52.0,10.0],
             "properties":{{"datetime":"2026-01-01T00:00:00Z"}},
             "links":[],"assets":{{}}}}"#
    )
}

/// Count the `rel: "item"` links of a written collection.
fn item_link_count(collection: &Path) -> usize {
    let written: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(collection).unwrap()).unwrap();
    written["links"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|l| l["rel"] == "item")
        .count()
}

/// `--items-dir` must find STAC Items by CONTENT, not by filename: CityParquet
/// names its Item `metadata.json`, while this tool's own convention is
/// `*_item.json`. Both must be picked up, nested directories included, and
/// `collection.json` must be ignored.
#[test]
fn items_dir_collects_items_by_content_recursively() {
    let tmp = tempfile::tempdir().unwrap();
    let items = tmp.path().join("items");
    fs::create_dir_all(items.join("a")).unwrap();
    fs::create_dir_all(items.join("b")).unwrap();

    fs::write(items.join("a/metadata.json"), item_json("a", 4.0)).unwrap();
    fs::write(items.join("b/b_item.json"), item_json("b", 5.0)).unwrap();
    // Must be ignored: a Collection, not an Item.
    fs::write(
        items.join("collection.json"),
        r#"{"type":"Collection","stac_version":"1.1.0","id":"c",
            "description":"x","license":"other","extent":{},"links":[]}"#,
    )
    .unwrap();

    let out = tmp.path().join("collection.json");
    let status = Command::new(env!("CARGO_BIN_EXE_city3dstac"))
        .args(["update-collection", "--items-dir"])
        .arg(&items)
        .args(["--id", "test-collection", "--description", "d", "-o"])
        .arg(&out)
        .status()
        .unwrap();
    assert!(status.success(), "update-collection must succeed");
    assert_eq!(item_link_count(&out), 2, "both items must be aggregated");
}

/// A symlink cycle inside `--items-dir` must not trap the walk.
///
/// `Path::is_dir` resolves through symlinks, so a link pointing at its own
/// ancestor turns the traversal into an unbounded descent (each level also
/// re-collects the real items, so any output would carry duplicates too). The
/// walk must therefore test the entry's own file type and not descend into
/// symlinked directories at all.
#[cfg(unix)]
#[test]
fn items_dir_terminates_on_symlink_cycle() {
    let tmp = tempfile::tempdir().unwrap();
    let items = tmp.path().join("items");
    fs::create_dir_all(items.join("a")).unwrap();
    fs::write(items.join("a/metadata.json"), item_json("a", 4.0)).unwrap();
    // The cycle: `items/loop` resolves back to `items` itself.
    std::os::unix::fs::symlink(".", items.join("loop")).unwrap();

    let out = tmp.path().join("collection.json");
    let child = Command::new(env!("CARGO_BIN_EXE_city3dstac"))
        .args(["update-collection", "--items-dir"])
        .arg(&items)
        .args(["--id", "test-collection", "--description", "d", "-o"])
        .arg(&out)
        .spawn()
        .unwrap();
    let status = wait_with_timeout(child, Duration::from_secs(30));

    assert!(status.success(), "update-collection must succeed");
    assert_eq!(
        item_link_count(&out),
        1,
        "the single real item must be collected exactly once"
    );
}

/// Wait for `child`, killing it and failing the test if it outlives `limit`.
///
/// A traversal caught in a symlink cycle neither succeeds nor fails quickly — it
/// descends until path lengths exceed `PATH_MAX` — so the test must impose its
/// own deadline rather than block on `Child::wait`.
#[cfg(unix)]
fn wait_with_timeout(mut child: std::process::Child, limit: Duration) -> std::process::ExitStatus {
    let start = Instant::now();
    loop {
        match child.try_wait().unwrap() {
            Some(status) => return status,
            None if start.elapsed() > limit => {
                let _ = child.kill();
                let _ = child.wait();
                panic!("update-collection did not terminate within {limit:?}: the --items-dir walk followed a symlink cycle");
            }
            None => std::thread::sleep(Duration::from_millis(50)),
        }
    }
}
