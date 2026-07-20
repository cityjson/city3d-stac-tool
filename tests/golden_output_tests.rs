//! Golden-output tests: the crate split must not change a byte of generated STAC.
//!
//! Regenerate deliberately with `UPDATE_GOLDEN=1 cargo test --test golden_output_tests`.
//! Never regenerate to make a refactor pass — that defeats the point of this file.

use city3d_stac::reader::get_reader;
use city3d_stac::stac::StacItemBuilder;
use std::path::{Path, PathBuf};

fn golden_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden")
}

/// Compare `actual` against the stored golden file, or write it when
/// `UPDATE_GOLDEN=1` is set.
fn assert_golden(name: &str, actual: &str) {
    let path = golden_dir().join(name);
    if std::env::var("UPDATE_GOLDEN").is_ok() {
        std::fs::create_dir_all(golden_dir()).unwrap();
        std::fs::write(&path, actual).unwrap();
        return;
    }
    let expected = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("missing golden file {}: {e}", path.display()));
    assert_eq!(
        expected, actual,
        "generated output for {name} changed; the refactor is not behaviour-preserving"
    );
}

/// Build an item from a fixture exactly the way the CLI's `item` command does,
/// then serialise it deterministically.
fn item_json(fixture: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data")
        .join(fixture);
    let reader = get_reader(&path).expect("reader");
    let item = StacItemBuilder::from_file(&path, reader.as_ref(), None, None)
        .expect("builder")
        .build()
        .expect("build");
    let mut json = serde_json::to_string_pretty(&item).expect("serialise");
    json.push('\n');
    json
}

#[test]
fn golden_item_delft_cityjson() {
    assert_golden("item_delft.city.json.json", &item_json("delft.city.json"));
}

#[test]
fn golden_item_delft_cityjsonseq() {
    assert_golden(
        "item_delft.city.jsonl.json",
        &item_json("delft.city.jsonl"),
    );
}

#[test]
fn golden_item_railway_cityjson() {
    assert_golden("item_railway.city.json.json", &item_json("railway.city.json"));
}

#[test]
fn golden_item_citygml2() {
    assert_golden("item_3dbag_citygml2.gml.json", &item_json("3dbag_citygml2.gml"));
}

#[test]
fn golden_item_citygml3() {
    assert_golden("item_3dbag_citygml3.gml.json", &item_json("3dbag_citygml3.gml"));
}

#[test]
fn golden_item_fcb() {
    assert_golden("item_all.fcb.json", &item_json("all.fcb"));
}
