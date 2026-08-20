use city3d_stac_types::stac::ItemMetadata;

#[test]
fn item_metadata_round_trips_through_json() {
    let meta = ItemMetadata {
        id: "delft".to_string(),
        bbox: Some(vec![4.3, 51.9, 0.0, 4.4, 52.0, 20.0]),
        city3d_version: Some("2.0".to_string()),
        ..Default::default()
    };
    let json = serde_json::to_string(&meta).unwrap();
    let back: ItemMetadata = serde_json::from_str(&json).unwrap();
    assert_eq!(back.id, "delft");
    assert_eq!(back.city3d_version.as_deref(), Some("2.0"));
}
