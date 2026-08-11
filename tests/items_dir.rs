use std::fs;
use std::process::Command;

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

    // A minimal but valid Item: the collection builder aggregates the spatial and
    // temporal extents from the items, so `bbox` and `datetime` must be real.
    let item = |id: &str, x: f64| {
        format!(
            r#"{{"type":"Feature","stac_version":"1.1.0","id":"{id}",
                 "geometry":{{"type":"Point","coordinates":[{x},52.0]}},
                 "bbox":[{x},52.0,0.0,{x},52.0,10.0],
                 "properties":{{"datetime":"2026-01-01T00:00:00Z"}},
                 "links":[],"assets":{{}}}}"#
        )
    };
    fs::write(items.join("a/metadata.json"), item("a", 4.0)).unwrap();
    fs::write(items.join("b/b_item.json"), item("b", 5.0)).unwrap();
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

    let written: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&out).unwrap()).unwrap();
    let item_links: Vec<_> = written["links"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|l| l["rel"] == "item")
        .collect();
    assert_eq!(
        item_links.len(),
        2,
        "both items must be aggregated: {written}"
    );
}
