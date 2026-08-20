//! `update-collection` item links must be resolvable from the collection.
//!
//! An item's href is derived from its path *relative to the directory holding
//! `collection.json`*, so a nested layout (`items/<id>/metadata.json`, which is
//! what CityParquet writes) yields a link that actually resolves. The flat
//! `*_item.json` layout the registry repo relies on is unaffected: there the
//! relative path is the bare filename it always was.

use std::fs;
use std::path::Path;
use std::process::Command;

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

/// The `rel: "item"` links of a written collection, as (href, title) pairs.
fn item_links(collection: &Path) -> Vec<(String, String)> {
    let written: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(collection).unwrap()).unwrap();
    written["links"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|l| l["rel"] == "item")
        .map(|l| {
            (
                l["href"].as_str().unwrap().to_string(),
                l["title"].as_str().unwrap_or_default().to_string(),
            )
        })
        .collect()
}

fn run(args: &[&std::ffi::OsStr]) {
    let status = Command::new(env!("CARGO_BIN_EXE_city3dstac"))
        .args(args)
        .status()
        .unwrap();
    assert!(status.success(), "update-collection must succeed");
}

/// Backward compatibility: the registry repo passes `*_item.json` files that
/// sit beside the collection, and their links must stay bare filenames.
#[test]
fn flat_layout_hrefs_stay_bare_filenames() {
    let tmp = tempfile::tempdir().unwrap();
    fs::write(tmp.path().join("a_item.json"), item_json("a", 4.0)).unwrap();
    fs::write(tmp.path().join("b_item.json"), item_json("b", 5.0)).unwrap();
    let out = tmp.path().join("collection.json");

    run(&[
        "update-collection".as_ref(),
        tmp.path().join("a_item.json").as_ref(),
        tmp.path().join("b_item.json").as_ref(),
        "--id".as_ref(),
        "flat".as_ref(),
        "--description".as_ref(),
        "d".as_ref(),
        "-o".as_ref(),
        out.as_ref(),
    ]);

    assert_eq!(
        item_links(&out),
        vec![
            ("./a_item.json".to_string(), "a".to_string()),
            ("./b_item.json".to_string(), "b".to_string()),
        ]
    );
}

/// The layout CityParquet writes: one directory per item, each holding a
/// `metadata.json`. Deriving the href from the filename alone made every link
/// `./metadata.json`, resolving beside the collection where no such file is.
#[test]
fn nested_layout_hrefs_are_relative_to_the_collection() {
    let tmp = tempfile::tempdir().unwrap();
    let items = tmp.path().join("items");
    fs::create_dir_all(items.join("a")).unwrap();
    fs::create_dir_all(items.join("b")).unwrap();
    fs::write(items.join("a/metadata.json"), item_json("a", 4.0)).unwrap();
    fs::write(items.join("b/metadata.json"), item_json("b", 5.0)).unwrap();
    let out = tmp.path().join("collection.json");

    run(&[
        "update-collection".as_ref(),
        "--items-dir".as_ref(),
        items.as_ref(),
        "--id".as_ref(),
        "nested".as_ref(),
        "--description".as_ref(),
        "d".as_ref(),
        "-o".as_ref(),
        out.as_ref(),
    ]);

    assert_eq!(
        item_links(&out),
        vec![
            ("./items/a/metadata.json".to_string(), "a".to_string()),
            ("./items/b/metadata.json".to_string(), "b".to_string()),
        ]
    );
}

/// `--items-base-url` joins the same relative path, so a published mirror's
/// absolute links keep the nesting too.
#[test]
fn items_base_url_joins_the_relative_path() {
    let tmp = tempfile::tempdir().unwrap();
    let items = tmp.path().join("items");
    fs::create_dir_all(items.join("a")).unwrap();
    fs::write(items.join("a/metadata.json"), item_json("a", 4.0)).unwrap();
    let out = tmp.path().join("collection.json");

    run(&[
        "update-collection".as_ref(),
        "--items-dir".as_ref(),
        items.as_ref(),
        "--items-base-url".as_ref(),
        "https://example.com/c".as_ref(),
        "--id".as_ref(),
        "nested".as_ref(),
        "--description".as_ref(),
        "d".as_ref(),
        "-o".as_ref(),
        out.as_ref(),
    ]);

    assert_eq!(
        item_links(&out),
        vec![(
            "https://example.com/c/items/a/metadata.json".to_string(),
            "a".to_string()
        )]
    );
}

/// An item outside the collection's directory has no relative path to offer, so
/// the href falls back to the bare filename it has always been.
#[test]
fn an_item_outside_the_collection_directory_falls_back_to_its_filename() {
    let tmp = tempfile::tempdir().unwrap();
    let elsewhere = tmp.path().join("elsewhere");
    fs::create_dir_all(&elsewhere).unwrap();
    fs::write(elsewhere.join("a_item.json"), item_json("a", 4.0)).unwrap();
    let out = tmp.path().join("out/collection.json");

    run(&[
        "update-collection".as_ref(),
        elsewhere.join("a_item.json").as_ref(),
        "--id".as_ref(),
        "outside".as_ref(),
        "--description".as_ref(),
        "d".as_ref(),
        "-o".as_ref(),
        out.as_ref(),
    ]);

    assert_eq!(
        item_links(&out),
        vec![("./a_item.json".to_string(), "a".to_string())]
    );
}

/// A file that survives `--items-dir`'s content sniff but fails to parse as an
/// Item is skipped (`--skip-errors` is on by default). Pairing the *input*
/// paths against the *parsed* items then shifted every later link onto the
/// wrong path, so each surviving item must be checked against its own file.
#[test]
fn a_skipped_item_does_not_misalign_the_remaining_links() {
    let tmp = tempfile::tempdir().unwrap();
    let items = tmp.path().join("items");
    fs::create_dir_all(items.join("zzz")).unwrap();
    // Sorts before `zzz/`, so it is the skip that would shift the rest. The
    // sniff only checks that `stac_version` is present; a non-string one still
    // fails to deserialise into an Item.
    fs::write(
        items.join("aaa_broken.json"),
        r#"{"type":"Feature","stac_version":11,"id":"broken","geometry":null,
            "properties":{},"links":[],"assets":{}}"#,
    )
    .unwrap();
    fs::write(items.join("zzz/metadata.json"), item_json("zzz", 4.0)).unwrap();
    let out = tmp.path().join("collection.json");

    run(&[
        "update-collection".as_ref(),
        "--items-dir".as_ref(),
        items.as_ref(),
        "--id".as_ref(),
        "skipped".as_ref(),
        "--description".as_ref(),
        "d".as_ref(),
        "-o".as_ref(),
        out.as_ref(),
    ]);

    assert_eq!(
        item_links(&out),
        vec![("./items/zzz/metadata.json".to_string(), "zzz".to_string())]
    );
}
