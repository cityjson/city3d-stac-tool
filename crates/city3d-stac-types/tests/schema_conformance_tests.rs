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

/// Boundary case, updated after Round B item 6: an Item built without ever
/// calling `.city3d(...)` carries none of the `city3d:*` properties the
/// schema's `require_any_field` definition demands at least one of.
/// `build()` used to push `CITY3D_EXTENSION` into `stac_extensions`
/// unconditionally, which made this Item schema-invalid (declaring an
/// extension it satisfied none of the required fields for) — see this
/// round's report, item 4, for the original rejection.
///
/// `build()` now mirrors how the projection extension is already
/// conditioned on `proj:code` being present: it only declares
/// `CITY3D_EXTENSION` when at least one `city3d:*` property exists. This
/// test pins that: an Item with no `city3d:*` properties declares no city3d
/// extension at all.
///
/// It does **not** additionally assert that the Item validates against
/// `vendored_schema()`, because it still won't, and that is correct: the
/// vendored schema is the city3d extension's *own* conformance schema, not
/// a generic STAC-Item schema. Both its `oneOf` branches (Item, Collection)
/// pull in `#/definitions/stac_extensions`, which makes `stac_extensions`
/// a *required* property whose value must `contains` the city3d URL —
/// unconditionally, regardless of whether the document actually uses the
/// extension. So this schema can only ever validate documents that declare
/// the extension; a plain Item that doesn't use it is out of this schema's
/// domain entirely; failing it is not a defect in the Item, just the wrong
/// schema to check a non-participating document against (real
/// extension-conformance tooling gates on `stac_extensions.contains(url)`
/// before ever invoking the extension's schema, for the same reason).
#[test]
fn an_item_without_any_city3d_fields_omits_the_extension() {
    let item = StacItemBuilder::new("schema-conformance-no-city3d-fields")
        .build()
        .unwrap();

    assert!(
        !item.extensions.iter().any(|e| e == CITY3D_EXTENSION),
        "an Item with no city3d:* properties must not declare the city3d extension"
    );
}

/// Companion to the boundary case above: an Item that *does* set at least
/// one `city3d:*` property must still declare the extension. Together the
/// two tests pin `build()`'s conditional-declaration rule in both
/// directions.
#[test]
fn an_item_with_city3d_fields_still_declares_the_extension() {
    let item = StacItemBuilder::new("schema-conformance-with-city3d-fields")
        .city3d(City3dProperties {
            version: Some("2.0".to_string()),
            ..Default::default()
        })
        .unwrap()
        .build()
        .unwrap();

    assert!(
        item.extensions.iter().any(|e| e == CITY3D_EXTENSION),
        "an Item with at least one city3d:* property must declare the city3d extension"
    );
}
