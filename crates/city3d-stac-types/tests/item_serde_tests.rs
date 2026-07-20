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

    // Key order is load-bearing for the golden fixtures elsewhere in this
    // workspace, and is only insertion-ordered because `serde_json` is built
    // with `preserve_order`. Without that feature `serde_json::Map` falls
    // back to a `BTreeMap`, which would alphabetise these keys instead
    // (assets, bbox, geometry, id, links, properties, stac_extensions,
    // stac_version, type) — this assertion is what catches that regression.
    let keys: Vec<&str> = json
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        keys,
        vec![
            "type",
            "stac_version",
            "stac_extensions",
            "id",
            "geometry",
            "bbox",
            "properties",
            "links",
            "assets",
        ],
        "top-level key order must match declaration order in Item (requires serde_json/preserve_order)"
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

/// Upstream `stac::Link` carries a flattened `additional_fields`. Ours must
/// too, or foreign members on links are silently dropped when a user-supplied
/// Item is read and written back out.
#[test]
fn unknown_link_fields_round_trip() {
    let json = serde_json::json!({
        "href": "./a.json",
        "rel": "alternate",
        "type": "application/json",
        "title": "A",
        "method": "GET",
        "cityjson:note": "keep me"
    });
    let link: Link = serde_json::from_value(json.clone()).unwrap();
    let back = serde_json::to_value(&link).unwrap();
    assert_eq!(back["method"], "GET");
    assert_eq!(back["cityjson:note"], "keep me");
    assert_eq!(back, json);
}

/// Upstream deserialises `datetime` permissively: RFC 3339 first, falling back
/// to a naive datetime assumed to be UTC. Items the CLI accepted before the
/// local model existed must keep parsing.
#[test]
fn naive_datetimes_are_accepted() {
    for (raw, expected) in [
        ("2024-01-15T12:00:00Z", "2024-01-15T12:00:00Z"),
        ("2024-01-15T12:00:00+01:00", "2024-01-15T11:00:00Z"),
        // No timezone at all — rejected by RFC 3339, accepted by upstream.
        ("2024-01-15T12:00:00", "2024-01-15T12:00:00Z"),
        ("2024-01-15T12:00:00.500", "2024-01-15T12:00:00.500Z"),
    ] {
        let item: Item = serde_json::from_value(serde_json::json!({
            "type": "Feature",
            "stac_version": "1.1.0",
            "id": "dt",
            "geometry": null,
            "properties": {"datetime": raw, "start_datetime": raw, "end_datetime": raw},
            "links": [],
            "assets": {}
        }))
        .unwrap_or_else(|e| panic!("{raw} should parse: {e}"));

        let json = serde_json::to_value(&item).unwrap();
        assert_eq!(json["properties"]["datetime"], expected, "input {raw}");
        assert_eq!(
            json["properties"]["start_datetime"], expected,
            "input {raw}"
        );
        assert_eq!(json["properties"]["end_datetime"], expected, "input {raw}");
    }
}

#[test]
fn unparseable_datetimes_are_still_rejected() {
    let result: Result<Item, _> = serde_json::from_value(serde_json::json!({
        "type": "Feature",
        "stac_version": "1.1.0",
        "id": "dt",
        "geometry": null,
        "properties": {"datetime": "not-a-datetime"},
        "links": [],
        "assets": {}
    }));
    assert!(result.is_err());
}
