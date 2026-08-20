//! End-to-end tests for CityJSONSequence
use assert_cmd::Command;
use std::path::Path;
use tempfile::tempdir;

/// Test data directory path
fn test_data_path(filename: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("data")
        .join(filename)
}

#[test]
fn test_cjseq_item_generation() {
    let input_path = test_data_path("delft.city.jsonl");
    let temp = tempdir().expect("Failed to create temp dir");
    let output_path = temp.path().join("item.json");

    // Run CLI
    let mut cmd = Command::cargo_bin("city3dstac").unwrap();
    cmd.args([
        "item",
        input_path.to_str().unwrap(),
        "-o",
        output_path.to_str().unwrap(),
    ])
    .assert()
    .success();

    // Verify output
    assert!(output_path.exists());
    let content = std::fs::read_to_string(&output_path).expect("Failed to read output");

    // Check for STAC properties
    assert!(content.contains("stac_version"));
    assert!(content.contains("id"));

    // Check for correct projection and metadata
    assert!(content.contains("proj:code"));
    assert!(content.contains("EPSG:7415"));
    assert!(content.contains("city3d:version"));

    let item: serde_json::Value = serde_json::from_str(&content).unwrap();

    assert_eq!(item["properties"]["proj:code"], "EPSG:7415");
    assert_eq!(item["properties"]["city3d:version"], "2.0");
    // delft.city.jsonl is the real 3DBAG fixture: 159 CityJSONFeature lines,
    // each a Building with one BuildingPart child, so 319 CityObjects total.
    assert_eq!(item["properties"]["city3d:city_objects"], 319);
    assert_eq!(
        item["properties"]["city3d:co_types"],
        serde_json::json!(["Building", "BuildingPart"])
    );
}
