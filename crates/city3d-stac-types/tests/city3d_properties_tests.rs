use city3d_stac_types::metadata::{AttributeDefinition, AttributeType};
use city3d_stac_types::stac::{City3dProperties, CityObjectsCount};

#[test]
fn writes_every_populated_field_with_the_city3d_prefix() {
    let props = City3dProperties {
        version: Some("2.0".to_string()),
        lods: vec!["1.2".to_string(), "2.2".to_string()],
        co_types: vec!["Building".to_string()],
        city_objects: Some(CityObjectsCount::Integer(42)),
        semantic_surfaces: Some(true),
        textures: Some(false),
        materials: Some(false),
        attributes: vec![AttributeDefinition::new("height", AttributeType::Number)],
    };

    let mut map = serde_json::Map::new();
    props.write_into(&mut map).unwrap();

    assert_eq!(map["city3d:version"], "2.0");
    assert_eq!(map["city3d:lods"], serde_json::json!(["1.2", "2.2"]));
    assert_eq!(map["city3d:co_types"], serde_json::json!(["Building"]));
    assert_eq!(map["city3d:city_objects"], 42);
    assert_eq!(map["city3d:semantic_surfaces"], true);
    assert_eq!(map["city3d:textures"], false);
    assert_eq!(map["city3d:materials"], false);
    assert_eq!(map["city3d:attributes"][0]["name"], "height");
}

#[test]
fn omits_empty_collections_but_keeps_false_booleans() {
    // Mirrors the existing builder: empty lods/co_types/attributes are skipped
    // entirely, while a `false` boolean is a meaningful assertion and is kept.
    let props = City3dProperties {
        version: None,
        lods: vec![],
        co_types: vec![],
        city_objects: None,
        semantic_surfaces: Some(false),
        textures: None,
        materials: None,
        attributes: vec![],
    };

    let mut map = serde_json::Map::new();
    props.write_into(&mut map).unwrap();

    assert!(!map.contains_key("city3d:lods"));
    assert!(!map.contains_key("city3d:co_types"));
    assert!(!map.contains_key("city3d:attributes"));
    assert!(!map.contains_key("city3d:version"));
    assert!(!map.contains_key("city3d:city_objects"));
    assert_eq!(map["city3d:semantic_surfaces"], false);
    assert_eq!(map.len(), 1);
}
