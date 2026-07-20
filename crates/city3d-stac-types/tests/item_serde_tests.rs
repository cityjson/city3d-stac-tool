use city3d_stac_types::stac::types::{Asset, Item, Link};

#[test]
fn item_serialises_with_stac_field_names_and_order() {
    let mut item = Item::new("delft");
    item.bbox = Some(vec![4.3, 51.9, 0.0, 4.4, 52.0, 20.0]);
    item.geometry = Some(serde_json::json!({"type": "Point", "coordinates": [4.35, 51.95]}));
    item.properties
        .additional_fields
        .insert("city3d:version".to_string(), serde_json::json!("2.0"));
    item.extensions = vec!["https://cityjson.github.io/stac-city3d/v0.2.0/schema.json".to_string()];
    item.assets.insert(
        "data".to_string(),
        Asset::new("./delft.city.json").with_media_type("application/city+json"),
    );
    item.links.push(Link::collection("../collection.json"));

    let json = serde_json::to_value(&item).unwrap();

    assert_eq!(json["type"], "Feature");
    assert_eq!(json["stac_version"], "1.1.0");
    assert_eq!(json["id"], "delft");
    assert_eq!(json["properties"]["city3d:version"], "2.0");
    assert_eq!(json["assets"]["data"]["href"], "./delft.city.json");
    assert_eq!(json["links"][0]["rel"], "collection");
    assert_eq!(
        json["stac_extensions"][0],
        "https://cityjson.github.io/stac-city3d/v0.2.0/schema.json"
    );
}

#[test]
fn absent_optional_fields_are_omitted_not_null() {
    let item = Item::new("bare");
    let json = serde_json::to_value(&item).unwrap();
    assert!(!json.as_object().unwrap().contains_key("bbox"));
    assert!(!json.as_object().unwrap().contains_key("collection"));
    // `datetime` is the exception: STAC requires the key even when null.
    assert!(json["properties"]
        .as_object()
        .unwrap()
        .contains_key("datetime"));
    assert!(json["properties"]["datetime"].is_null());
}

#[test]
fn item_round_trips_through_json() {
    let mut item = Item::new("rt");
    item.bbox = Some(vec![0.0, 0.0, 1.0, 1.0]);
    let json = serde_json::to_string(&item).unwrap();
    let back: Item = serde_json::from_str(&json).unwrap();
    assert_eq!(back.id, "rt");
    assert_eq!(back.bbox, Some(vec![0.0, 0.0, 1.0, 1.0]));
}

/// STAC requires `geometry`, `links` and `assets` to be present on every Item,
/// and the upstream `stac` crate serialises all three unconditionally. The
/// local model must do the same or schema validation of a bare Item breaks.
#[test]
fn required_stac_keys_are_always_present() {
    let item = Item::new("bare");
    let json = serde_json::to_value(&item).unwrap();
    let obj = json.as_object().unwrap();
    assert!(obj.contains_key("geometry"));
    assert!(json["geometry"].is_null());
    assert_eq!(json["links"], serde_json::json!([]));
    assert_eq!(json["assets"], serde_json::json!({}));
    // `stac_extensions` is the one upstream omits when empty.
    assert!(!obj.contains_key("stac_extensions"));
}

/// Unknown top-level members must survive a JSON round-trip, exactly as they
/// do on the upstream type, so the interop conversion is lossless.
#[test]
fn unknown_top_level_fields_round_trip() {
    let json = serde_json::json!({
        "type": "Feature",
        "stac_version": "1.1.0",
        "id": "extra",
        "geometry": null,
        "properties": {"datetime": null},
        "links": [],
        "assets": {},
        "some_foreign_member": {"a": 1}
    });
    let item: Item = serde_json::from_value(json).unwrap();
    let back = serde_json::to_value(&item).unwrap();
    assert_eq!(back["some_foreign_member"], serde_json::json!({"a": 1}));
}
