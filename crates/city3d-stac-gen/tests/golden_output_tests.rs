//! Golden-output tests: the crate split must not change a byte of generated STAC.
//!
//! Regenerate deliberately with `UPDATE_GOLDEN=1 cargo test --test golden_output_tests`.
//! Never regenerate to make a refactor pass — that defeats the point of this file.
//!
//! Floating-point values are the one exception to byte-for-byte equality:
//! reprojection goes through the platform's libm, so the last bits of a
//! coordinate differ between e.g. macOS and Linux. Floats are compared with a
//! tiny relative tolerance; structure, key order, strings and integers must
//! still match exactly.

use city3d_stac::reader::{get_reader, CityModelMetadataReader};
use city3d_stac::stac::{item_from_file, StacCollectionBuilder};
use city3d_stac::traversal::find_files_with_patterns;
use serde_json::Value;
use std::path::{Path, PathBuf};

/// Relative tolerance for float comparison: far below any meaningful change
/// (1e-12 of a degree is ~0.1 µm), far above cross-platform libm noise.
const FLOAT_REL_TOLERANCE: f64 = 1e-12;

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
    if expected == actual {
        return;
    }
    let expected_json: Value = serde_json::from_str(&expected).expect("golden file is JSON");
    let actual_json: Value = serde_json::from_str(actual).expect("generated output is JSON");
    if let Err(at) = json_close(&expected_json, &actual_json, "$") {
        assert_eq!(
            expected, actual,
            "generated output for {name} changed at {at}; the refactor is not behaviour-preserving"
        );
    }
}

/// Structural JSON equality with float tolerance. Object key order is
/// significant. Returns the JSON path of the first difference.
fn json_close(expected: &Value, actual: &Value, at: &str) -> Result<(), String> {
    match (expected, actual) {
        (Value::Number(e), Value::Number(a)) => {
            if e == a {
                return Ok(());
            }
            let both_float = e.is_f64() && a.is_f64();
            let (e, a) = (e.as_f64().unwrap(), a.as_f64().unwrap());
            let scale = e.abs().max(a.abs()).max(1.0);
            if both_float && (e - a).abs() <= FLOAT_REL_TOLERANCE * scale {
                Ok(())
            } else {
                Err(at.to_string())
            }
        }
        (Value::Array(e), Value::Array(a)) => {
            if e.len() != a.len() {
                return Err(at.to_string());
            }
            e.iter()
                .zip(a)
                .enumerate()
                .try_for_each(|(i, (e, a))| json_close(e, a, &format!("{at}[{i}]")))
        }
        (Value::Object(e), Value::Object(a)) => {
            if !e.keys().eq(a.keys()) {
                return Err(at.to_string());
            }
            e.iter()
                .zip(a.values())
                .try_for_each(|((k, e), a)| json_close(e, a, &format!("{at}.{k}")))
        }
        (e, a) if e == a => Ok(()),
        _ => Err(at.to_string()),
    }
}

#[test]
fn json_close_tolerates_only_float_noise() {
    let golden: Value = serde_json::from_str(r#"{"a": [4.366889721271404, 1], "b": "x"}"#).unwrap();
    let noise: Value = serde_json::from_str(r#"{"a": [4.366889721271403, 1], "b": "x"}"#).unwrap();
    let moved: Value = serde_json::from_str(r#"{"a": [4.3668897213, 1], "b": "x"}"#).unwrap();
    let int: Value = serde_json::from_str(r#"{"a": [4.366889721271404, 2], "b": "x"}"#).unwrap();
    let order: Value = serde_json::from_str(r#"{"b": "x", "a": [4.366889721271404, 1]}"#).unwrap();
    assert!(json_close(&golden, &noise, "$").is_ok());
    assert_eq!(json_close(&golden, &moved, "$"), Err("$.a[0]".into()));
    assert_eq!(json_close(&golden, &int, "$"), Err("$.a[1]".into()));
    assert_eq!(json_close(&golden, &order, "$"), Err("$".into()));
}

/// Build an item from a fixture exactly the way the CLI's `item` command does,
/// then serialise it deterministically.
fn item_json(fixture: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data")
        .join(fixture);
    let reader = get_reader(&path).expect("reader");
    let item = item_from_file(&path, reader.as_ref(), None, None)
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
    assert_golden("item_delft.city.jsonl.json", &item_json("delft.city.jsonl"));
}

#[test]
fn golden_item_railway_cityjson() {
    assert_golden(
        "item_railway.city.json.json",
        &item_json("railway.city.json"),
    );
}

#[test]
fn golden_item_citygml2() {
    assert_golden(
        "item_3dbag_citygml2.gml.json",
        &item_json("3dbag_citygml2.gml"),
    );
}

#[test]
fn golden_item_citygml3() {
    assert_golden(
        "item_3dbag_citygml3.gml.json",
        &item_json("3dbag_citygml3.gml"),
    );
}

#[test]
fn golden_item_fcb() {
    assert_golden("item_all.fcb.json", &item_json("all.fcb"));
}

#[test]
fn golden_item_with_datetime() {
    // The other six fixtures above all carry no `referenceDate`, so every
    // other golden Item pins `"datetime": null`. This is the one fixture
    // whose CityJSON metadata supplies a `referenceDate`, exercising the
    // non-null path through `datetime_from_reference_date` and the custom
    // permissive datetime (de)serialisation — one of the few things that
    // genuinely changed in the local-Item swap. Lives in a subdirectory so
    // the non-recursive collection scan below (max_depth 1) does not pick
    // it up and perturb `collection_test_data.json`.
    assert_golden(
        "item_with_reference_date.city.json.json",
        &item_json("datetime_fixture/with_reference_date.city.json"),
    );
}

/// Build a collection over the whole `tests/data` fixture directory the same way
/// the CLI's `collection` command discovers files (`find_files_with_patterns`,
/// non-recursive, no include/exclude filters — see `process_collection_logic` in
/// `src/cli/mod.rs`), aggregates their metadata through the public
/// `aggregate_cityjson_metadata` API (the same call `tests/integration_tests.rs`
/// and `tests/catalog_tests.rs` use), then serialises deterministically.
///
/// Most of this is deterministic: `find_files_with_patterns` sorts its result,
/// and `aggregate_cityjson_metadata` reduces most fields into sorted vectors or
/// via commutative min/max. No file paths, hostnames, or timestamps are
/// embedded anywhere in the output. But two fields inside `summaries` ARE
/// genuinely non-deterministic, both confirmed by diffing three independent
/// `UPDATE_GOLDEN=1` regenerations (same keys/values each time, different
/// order):
///
/// 1. **Key order of the `summaries` object itself.** `StacCollectionBuilder`
///    stores summaries in a `std::collections::HashMap` (`src/stac/collection.rs`),
///    and `.build()` copies it into the output `Map` by iterating that
///    `HashMap` without sorting. `HashMap`'s default hasher is seeded
///    per-process, so iteration order (and hence field order in the JSON)
///    varies run to run.
/// 2. **Element order of `summaries["city3d:version"]`.** Unlike every other
///    aggregated field (`city3d:lods`, `city3d:co_types`, `proj:code`, the
///    boolean summaries), `aggregate_cityjson_metadata`'s version aggregation
///    collects into a `HashSet<String>` and pushes it straight into a `Vec`
///    without a `.sort()` call — the one place that sort was seemingly
///    forgotten. This is a pre-existing quirk in `src/stac/collection.rs`, not
///    introduced by this test; it is out of scope to fix here since this file
///    exists to pin current behaviour, not change it.
///
/// We sort both before pinning so the golden file reflects real content, not
/// incidental hash-seed order — everything else keeps its natural
/// (deterministic) serialisation order untouched.
fn collection_json() -> String {
    let data_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data");
    let files = find_files_with_patterns(&[data_dir], &[], &[], false, None).expect("find files");
    let readers: Vec<Box<dyn CityModelMetadataReader>> = files
        .iter()
        .map(|f| get_reader(f).expect("reader"))
        .collect();
    let collection = StacCollectionBuilder::new("tests-data")
        .aggregate_cityjson_metadata(&readers)
        .expect("aggregate")
        .build()
        .expect("build");
    let mut value = serde_json::to_value(&collection).expect("to_value");
    if let Some(summaries) = value.get_mut("summaries").and_then(|s| s.as_object_mut()) {
        if let Some(versions) = summaries
            .get_mut("city3d:version")
            .and_then(|v| v.as_array_mut())
        {
            versions.sort_by(|a, b| a.as_str().cmp(&b.as_str()));
        }
        summaries.sort_keys();
    }
    let mut json = serde_json::to_string_pretty(&value).expect("serialise");
    json.push('\n');
    json
}

#[test]
fn golden_collection_over_test_data() {
    assert_golden("collection_test_data.json", &collection_json());
}
