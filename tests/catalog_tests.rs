use city3d_stac::config::CatalogConfigFile;
use city3d_stac::stac::StacCatalogBuilder;
use std::io::Write;
use tempfile::tempdir;

#[test]
fn test_catalog_config_from_toml() {
    let toml_content = r#"
        id = "test-catalog"
        title = "Test Catalog"
        description = "A test catalog"
        collections = ["col1", "col2"]
    "#;

    let dir = tempdir().unwrap();
    let file_path = dir.path().join("catalog.toml");
    let mut file = std::fs::File::create(&file_path).unwrap();
    file.write_all(toml_content.as_bytes()).unwrap();

    let config = CatalogConfigFile::from_file(&file_path).unwrap();

    assert_eq!(config.id, Some("test-catalog".to_string()));
    assert_eq!(config.title, Some("Test Catalog".to_string()));
    assert_eq!(config.description, Some("A test catalog".to_string()));
    assert_eq!(
        config.collections,
        Some(vec!["col1".to_string(), "col2".to_string()])
    );
}

#[test]
fn test_catalog_builder() {
    let catalog = StacCatalogBuilder::new("cat-id", "cat-desc")
        .title("My Catalog")
        .child_link("./child/collection.json", Some("Child Title".to_string()))
        .self_link("./catalog.json")
        .build();

    assert_eq!(catalog.id, "cat-id");
    assert_eq!(catalog.description, "cat-desc");
    assert_eq!(catalog.title, Some("My Catalog".to_string()));
    assert_eq!(catalog.links.len(), 2);

    let child_link = catalog.links.iter().find(|l| l.rel == "child").unwrap();
    assert_eq!(child_link.href, "./child/collection.json");
    assert_eq!(child_link.title, Some("Child Title".to_string()));
    assert_eq!(child_link.r#type, Some("application/json".to_string()));

    let self_link = catalog.links.iter().find(|l| l.rel == "self").unwrap();
    assert_eq!(self_link.href, "./catalog.json");
}

#[test]
fn test_catalog_serialization() {
    let catalog = StacCatalogBuilder::new("cat-json", "serialization test").build();
    let json = serde_json::to_string(&catalog).unwrap();

    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed["type"], "Catalog");
    assert_eq!(parsed["stac_version"], "1.1.0");
    assert_eq!(parsed["id"], "cat-json");
    assert_eq!(parsed["description"], "serialization test");
    assert!(parsed.get("extent").is_none()); // Catalog shouldn't have extent
    assert!(parsed.get("license").is_none()); // Catalog shouldn't have license
}

#[test]
fn test_cli_catalog_command() {
    use assert_cmd::Command;

    let dir = tempdir().unwrap();
    let data_dir = dir.path().join("data");
    std::fs::create_dir(&data_dir).unwrap();

    // Create a dummy CityJSON file
    let cityjson_content = r#"{
        "type": "CityJSON",
        "version": "1.1",
        "CityObjects": {},
        "vertices": [],
        "transform": {
            "scale": [0.001, 0.001, 0.001],
            "translate": [0.0, 0.0, 0.0]
        }
    }"#;
    std::fs::write(data_dir.join("test.city.json"), cityjson_content).unwrap();

    let output_dir = dir.path().join("catalog");

    // Run catalog command
    let mut cmd = Command::cargo_bin("city3dstac").unwrap();
    cmd.args([
        "catalog",
        data_dir.to_str().unwrap(),
        "-o",
        output_dir.to_str().unwrap(),
        "--id",
        "test-catalog",
        "--description",
        "Test Catalog",
    ])
    .assert()
    .success();

    // Verify catalog.json exists
    let catalog_path = output_dir.join("catalog.json");
    assert!(catalog_path.exists());

    let catalog_content = std::fs::read_to_string(&catalog_path).unwrap();
    let catalog_json: serde_json::Value = serde_json::from_str(&catalog_content).unwrap();

    assert_eq!(catalog_json["id"], "test-catalog");
    assert_eq!(catalog_json["description"], "Test Catalog");

    // Verify sub-collection exists
    // The collection directory name should be "data" (from the input directory name)
    let collection_dir = output_dir.join("data");
    assert!(collection_dir.exists());

    let collection_path = collection_dir.join("collection.json");
    assert!(collection_path.exists());
}

#[test]
fn test_cli_catalog_refreshes_parent_root_links_on_existing_collection() {
    use assert_cmd::Command;

    let dir = tempdir().unwrap();
    let data_dir = dir.path().join("data");
    std::fs::create_dir(&data_dir).unwrap();

    let cityjson_content = r#"{
        "type": "CityJSON",
        "version": "1.1",
        "CityObjects": {},
        "vertices": [],
        "transform": {
            "scale": [0.001, 0.001, 0.001],
            "translate": [0.0, 0.0, 0.0]
        }
    }"#;
    std::fs::write(data_dir.join("test.city.json"), cityjson_content).unwrap();

    // First, generate a collection standalone (no catalog membership) so it
    // ends up without parent/root links — mirroring registries with pre-existing
    // collections generated before catalog membership was wired up.
    let collection_dir = dir.path().join("standalone-collection");
    Command::cargo_bin("city3dstac")
        .unwrap()
        .args([
            "collection",
            data_dir.to_str().unwrap(),
            "-o",
            collection_dir.to_str().unwrap(),
            "--id",
            "data",
        ])
        .assert()
        .success();

    let pre_links: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(collection_dir.join("collection.json")).unwrap(),
    )
    .unwrap();
    let pre_rels: Vec<&str> = pre_links["links"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|l| l["rel"].as_str())
        .collect();
    assert!(!pre_rels.contains(&"parent"));
    assert!(!pre_rels.contains(&"root"));

    // Now stage that collection inside a catalog output dir and run `catalog`
    // WITHOUT --overwrite. The existing collection.json should be preserved
    // but updated to include parent/root links pointing at ../catalog.json.
    let catalog_out = dir.path().join("catalog");
    std::fs::create_dir(&catalog_out).unwrap();
    let staged = catalog_out.join("data");
    std::fs::create_dir(&staged).unwrap();
    std::fs::copy(
        collection_dir.join("collection.json"),
        staged.join("collection.json"),
    )
    .unwrap();

    Command::cargo_bin("city3dstac")
        .unwrap()
        .args([
            "catalog",
            data_dir.to_str().unwrap(),
            "-o",
            catalog_out.to_str().unwrap(),
            "--id",
            "test-catalog",
            "--description",
            "Test Catalog",
        ])
        .assert()
        .success();

    let after: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(staged.join("collection.json")).unwrap())
            .unwrap();
    let links = after["links"].as_array().unwrap();
    let parent = links
        .iter()
        .find(|l| l["rel"] == "parent")
        .expect("parent link should be present");
    assert_eq!(parent["href"], "../catalog.json");
    let root = links
        .iter()
        .find(|l| l["rel"] == "root")
        .expect("root link should be present");
    assert_eq!(root["href"], "../catalog.json");
    // Only one of each — running again should not duplicate them.
    assert_eq!(links.iter().filter(|l| l["rel"] == "parent").count(), 1);
    assert_eq!(links.iter().filter(|l| l["rel"] == "root").count(), 1);
}

#[test]
fn test_cli_catalog_refreshes_parent_root_when_input_dir_missing() {
    // Scenario: the catalog config points at a collection input that no longer
    // exists, but a stale collection.json from a prior run is staged in the
    // catalog output dir. Per-collection processing returns Err for that
    // entry — we still want parent/root to land on the staged file because
    // the catalog handler refreshes links at the catalog level.
    use assert_cmd::Command;

    let dir = tempdir().unwrap();

    let catalog_out = dir.path().join("catalog");
    std::fs::create_dir(&catalog_out).unwrap();
    let staged = catalog_out.join("ghost-collection");
    std::fs::create_dir(&staged).unwrap();

    let stale_collection = serde_json::json!({
        "type": "Collection",
        "stac_version": "1.1.0",
        "id": "ghost-collection",
        "description": "Stale collection from a prior run",
        "license": "proprietary",
        "extent": {
            "spatial": { "bbox": [[0.0, 0.0, 1.0, 1.0]] },
            "temporal": { "interval": [["2020-01-01T00:00:00Z", null]] }
        },
        "links": [{
            "rel": "self",
            "href": "./collection.json",
            "type": "application/json"
        }]
    });
    std::fs::write(
        staged.join("collection.json"),
        serde_json::to_string_pretty(&stale_collection).unwrap(),
    )
    .unwrap();

    // Pass a non-existent input path whose filename matches the staged
    // collection's id_hint. process_collection_logic will short-circuit with
    // "Directory not found" — but the catalog-level pass should still update
    // parent/root on the staged file.
    let missing_input = dir.path().join("ghost-collection");
    assert!(!missing_input.exists());

    Command::cargo_bin("city3dstac")
        .unwrap()
        .args([
            "catalog",
            missing_input.to_str().unwrap(),
            "-o",
            catalog_out.to_str().unwrap(),
            "--id",
            "test-catalog",
            "--description",
            "Test Catalog",
        ])
        .assert()
        .success();

    let after: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(staged.join("collection.json")).unwrap())
            .unwrap();
    let links = after["links"].as_array().unwrap();
    let parent = links
        .iter()
        .find(|l| l["rel"] == "parent")
        .expect("parent link should be present even when input dir is missing");
    assert_eq!(parent["href"], "../catalog.json");
    let root = links
        .iter()
        .find(|l| l["rel"] == "root")
        .expect("root link should be present even when input dir is missing");
    assert_eq!(root["href"], "../catalog.json");
    assert_eq!(links.iter().filter(|l| l["rel"] == "parent").count(), 1);
    assert_eq!(links.iter().filter(|l| l["rel"] == "root").count(), 1);
}

#[test]
fn test_cli_catalog_reattaches_geoparquet_asset_on_existing_collection() {
    // Scenario: a staged collection.json sits next to an `items.parquet`
    // sibling but is missing the collection-level `items-geoparquet` asset
    // (e.g., produced by an older tool version or a partial regenerate path).
    // Running `catalog` should reconcile the asset reference so consumers
    // can discover the parquet mirror without re-running with `--geoparquet`.
    use assert_cmd::Command;

    let dir = tempdir().unwrap();

    let catalog_out = dir.path().join("catalog");
    std::fs::create_dir(&catalog_out).unwrap();
    let staged = catalog_out.join("ghost-collection");
    std::fs::create_dir(&staged).unwrap();

    let stale_collection = serde_json::json!({
        "type": "Collection",
        "stac_version": "1.1.0",
        "id": "ghost-collection",
        "description": "Stale collection from a prior run",
        "license": "proprietary",
        "extent": {
            "spatial": { "bbox": [[0.0, 0.0, 1.0, 1.0]] },
            "temporal": { "interval": [["2020-01-01T00:00:00Z", null]] }
        },
        "links": []
    });
    std::fs::write(
        staged.join("collection.json"),
        serde_json::to_string_pretty(&stale_collection).unwrap(),
    )
    .unwrap();
    // Sibling parquet — content is irrelevant for this test, only its presence.
    std::fs::write(staged.join("items.parquet"), b"PAR1").unwrap();

    let missing_input = dir.path().join("ghost-collection");

    Command::cargo_bin("city3dstac")
        .unwrap()
        .args([
            "catalog",
            missing_input.to_str().unwrap(),
            "-o",
            catalog_out.to_str().unwrap(),
            "--id",
            "test-catalog",
            "--description",
            "Test Catalog",
        ])
        .assert()
        .success();

    let after: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(staged.join("collection.json")).unwrap())
            .unwrap();
    let asset = after
        .get("assets")
        .and_then(|a| a.get("items-geoparquet"))
        .expect("items-geoparquet asset should be reattached when items.parquet is present");
    assert_eq!(asset["href"], "./items.parquet");
    assert_eq!(asset["type"], "application/vnd.apache.parquet");
    assert_eq!(asset["title"], "STAC GeoParquet items");
    let roles = asset["roles"].as_array().unwrap();
    assert!(roles.iter().any(|r| r == "collection-mirror"));
}

#[test]
fn test_cli_catalog_skips_geoparquet_asset_when_parquet_absent() {
    // Inverse of the above: no sibling parquet → no asset reference written.
    // Guards against writing a dangling href that points at a non-existent file.
    use assert_cmd::Command;

    let dir = tempdir().unwrap();

    let catalog_out = dir.path().join("catalog");
    std::fs::create_dir(&catalog_out).unwrap();
    let staged = catalog_out.join("ghost-collection");
    std::fs::create_dir(&staged).unwrap();

    let stale_collection = serde_json::json!({
        "type": "Collection",
        "stac_version": "1.1.0",
        "id": "ghost-collection",
        "description": "Stale collection from a prior run",
        "license": "proprietary",
        "extent": {
            "spatial": { "bbox": [[0.0, 0.0, 1.0, 1.0]] },
            "temporal": { "interval": [["2020-01-01T00:00:00Z", null]] }
        },
        "links": []
    });
    std::fs::write(
        staged.join("collection.json"),
        serde_json::to_string_pretty(&stale_collection).unwrap(),
    )
    .unwrap();

    let missing_input = dir.path().join("ghost-collection");

    Command::cargo_bin("city3dstac")
        .unwrap()
        .args([
            "catalog",
            missing_input.to_str().unwrap(),
            "-o",
            catalog_out.to_str().unwrap(),
            "--id",
            "test-catalog",
            "--description",
            "Test Catalog",
        ])
        .assert()
        .success();

    let after: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(staged.join("collection.json")).unwrap())
            .unwrap();
    let no_asset = after
        .get("assets")
        .and_then(|a| a.get("items-geoparquet"))
        .is_none();
    assert!(
        no_asset,
        "items-geoparquet asset must NOT be added when items.parquet is absent"
    );
}

#[test]
fn test_cli_collection_config_only_with_geoparquet() {
    use assert_cmd::Command;

    let dir = tempdir().unwrap();
    let config_path = dir.path().join("config-only.yaml");
    let yaml = r#"id: cfg-only
title: Config-only
description: Collection metadata without input items
license: CC-BY-4.0
inputs: []
extent:
  spatial:
    bbox: [4.0, 50.0, 5.0, 51.0]
    crs: EPSG:4326
  temporal:
    start: '2020-01-01T00:00:00Z'
"#;
    std::fs::write(&config_path, yaml).unwrap();

    let output_dir = dir.path().join("out");

    let mut cmd = Command::cargo_bin("city3dstac").unwrap();
    cmd.args([
        "collection",
        "-C",
        config_path.to_str().unwrap(),
        "-o",
        output_dir.to_str().unwrap(),
        "--geoparquet",
    ])
    .assert()
    .success();

    assert!(output_dir.join("collection.json").exists());
    assert!(!output_dir.join("items.parquet").exists());
}
