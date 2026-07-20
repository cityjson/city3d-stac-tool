//! The `interop` conversions claim to be lossless. Pin that claim.

use city3d_stac::stac::interop::{from_upstream, to_upstream};
use city3d_stac_types::stac::types::{Asset, Item, Link};

fn item_with_foreign_members() -> Item {
    let mut item = Item::new("interop");
    item.geometry = Some(serde_json::json!({"type": "Point", "coordinates": [4.35, 51.95]}));
    item.bbox = Some(vec![4.35, 51.95, 4.35, 51.95]);
    item.properties.datetime = Some("2024-01-15T12:00:00Z".parse().unwrap());
    item.properties
        .additional_fields
        .insert("city3d:version".to_string(), serde_json::json!("2.0"));
    item.additional_fields
        .insert("cityjson:top_level".to_string(), serde_json::json!(7));

    let mut asset = Asset::new("./data.city.json").with_media_type("application/city+json");
    asset
        .additional_fields
        .insert("file:size".to_string(), serde_json::json!(1234));
    item.assets.insert("data".to_string(), asset);

    let mut link = Link::new("./a.json", "alternate");
    link.media_type = Some("application/json".to_string());
    link.additional_fields
        .insert("cityjson:note".to_string(), serde_json::json!("keep me"));
    item.links.push(link);

    item
}

#[test]
fn interop_round_trip_preserves_every_field() {
    let item = item_with_foreign_members();

    let upstream = to_upstream(&item).expect("to_upstream");
    let back = from_upstream(&upstream).expect("from_upstream");

    assert_eq!(back, item, "round trip through upstream must be lossless");
}

#[test]
fn interop_preserves_foreign_members_in_json() {
    let item = item_with_foreign_members();
    let upstream_json = serde_json::to_value(to_upstream(&item).unwrap()).unwrap();

    assert_eq!(upstream_json["cityjson:top_level"], 7);
    assert_eq!(upstream_json["assets"]["data"]["file:size"], 1234);
    assert_eq!(upstream_json["links"][0]["cityjson:note"], "keep me");
    assert_eq!(
        serde_json::to_value(&item).unwrap(),
        upstream_json,
        "both models must serialise to the same STAC document"
    );
}
