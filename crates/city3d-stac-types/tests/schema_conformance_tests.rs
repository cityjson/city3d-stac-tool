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

/// `city3d:city_objects` is untagged: `CityObjectsCount::Integer` (a bare
/// number) or `CityObjectsCount::Statistics { min, max, total }` (an
/// object). The fully-populated test above only ever exercises `Integer`.
/// Cover `Statistics` too, so the schema's `city3d:city_objects` shape
/// (which allows both) is checked against both variants this crate can
/// actually emit.
#[test]
fn an_item_with_statistics_city_objects_count_validates_against_the_vendored_schema() {
    let props = City3dProperties {
        version: Some("2.0".to_string()),
        lods: vec!["2.2".to_string()],
        co_types: vec!["Building".to_string()],
        city_objects: Some(CityObjectsCount::Statistics {
            min: 1,
            max: 500,
            total: 2231,
        }),
        semantic_surfaces: Some(true),
        textures: Some(false),
        materials: Some(false),
        attributes: vec![AttributeDefinition::new("height", AttributeType::Number)],
    };
    let bbox = BBox3D {
        xmin: 4.3,
        ymin: 51.9,
        zmin: 0.0,
        xmax: 4.4,
        ymax: 52.0,
        zmax: 20.0,
    };

    let item = StacItemBuilder::new("schema-conformance-statistics")
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
        "emitted item with CityObjectsCount::Statistics violates the city3d schema:\n{}",
        errors.join("\n")
    );
}

/// Boundary case: an Item built without ever calling `.city3d(...)`, so it
/// carries none of the `city3d:*` properties the schema's
/// `require_any_field` definition demands at least one of. `build()`
/// unconditionally pushes `CITY3D_EXTENSION` into `stac_extensions`
/// regardless of whether `.city3d()` was called, so this Item declares the
/// extension while satisfying none of its required fields.
///
/// This is expected, and deliberately asserted, to FAIL validation: the
/// schema's `stac_extensions` definition requires the extension URL to be
/// present whenever declared (which `build()` always does), and separately
/// requires `require_any_field` whenever the extension is declared. An Item
/// with zero `city3d:*` fields cannot satisfy both. This test documents that
/// gap rather than papering over it — see this round's report for the exact
/// rejection and why fixing it (making the extension URL conditional on
/// having at least one `city3d:*` field) is left for a decision, since it
/// would change generated output for every Item that omits `city3d()`.
#[test]
fn an_item_without_any_city3d_fields_fails_the_require_any_field_rule() {
    let item = StacItemBuilder::new("schema-conformance-no-city3d-fields")
        .build()
        .unwrap();

    assert!(
        item.extensions.iter().any(|e| e == CITY3D_EXTENSION),
        "test premise: build() must still declare the city3d extension \
         even though .city3d() was never called"
    );

    let instance = serde_json::to_value(&item).unwrap();
    let schema = vendored_schema();
    let validator = jsonschema::validator_for(&schema).expect("compile schema");

    let errors: Vec<String> = validator
        .iter_errors(&instance)
        .map(|e| format!("{} at {}", e, e.instance_path))
        .collect();
    assert!(
        !errors.is_empty(),
        "expected the schema to reject an Item that declares the city3d \
         extension but carries none of its city3d:* fields; if this now \
         passes, build() has started making the extension URL conditional \
         and this test's premise needs revisiting"
    );
}
