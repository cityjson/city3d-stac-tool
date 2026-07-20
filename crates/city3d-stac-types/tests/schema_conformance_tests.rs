//! The vendored `stac-city3d` schema is the contract. If a schema release
//! outpaces `extensions::CITY3D_EXTENSION`, or if `City3dProperties` emits a
//! shape the schema rejects, that must fail here — in the crate that owns the
//! representation — not downstream in a consumer's output.

use city3d_stac_types::extensions::CITY3D_EXTENSION;
use city3d_stac_types::metadata::{AttributeDefinition, AttributeType, BBox3D};
use city3d_stac_types::stac::{City3dProperties, CityObjectsCount, StacItemBuilder};

fn vendored_schema() -> serde_json::Value {
    let text = include_str!("../schemas/stac-city3d-v0.2.0.json");
    serde_json::from_str(text).expect("vendored schema must be valid JSON")
}

#[test]
fn the_pinned_extension_url_matches_the_vendored_schema_id() {
    let schema = vendored_schema();
    assert_eq!(
        schema["$id"], CITY3D_EXTENSION,
        "extensions::CITY3D_EXTENSION has drifted from the vendored schema"
    );
}

#[test]
fn a_fully_populated_item_validates_against_the_vendored_schema() {
    let props = City3dProperties {
        version: Some("2.0".to_string()),
        lods: vec!["1.2".to_string(), "2.2".to_string()],
        co_types: vec!["Building".to_string(), "BuildingPart".to_string()],
        city_objects: Some(CityObjectsCount::Integer(2231)),
        semantic_surfaces: Some(true),
        textures: Some(false),
        materials: Some(false),
        attributes: vec![
            AttributeDefinition::new("height", AttributeType::Number),
            AttributeDefinition::new("identificatie", AttributeType::String),
        ],
    };
    let bbox = BBox3D {
        xmin: 4.3,
        ymin: 51.9,
        zmin: 0.0,
        xmax: 4.4,
        ymax: 52.0,
        zmax: 20.0,
    };

    let item = StacItemBuilder::new("schema-conformance")
        .bbox(bbox)
        .geometry_from_bbox()
        .city3d(props)
        .unwrap()
        .build()
        .unwrap();

    let instance = serde_json::to_value(&item).unwrap();
    let schema = vendored_schema();
    let validator = jsonschema::validator_for(&schema).expect("compile schema");

    let errors: Vec<String> = validator
        .iter_errors(&instance)
        .map(|e| format!("{} at {}", e, e.instance_path))
        .collect();
    assert!(
        errors.is_empty(),
        "emitted item violates the city3d schema:\n{}",
        errors.join("\n")
    );
}
