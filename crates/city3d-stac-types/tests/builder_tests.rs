use city3d_stac_types::metadata::{BBox3D, CRS};
use city3d_stac_types::stac::{City3dProperties, StacItemBuilder};

#[test]
fn builds_a_complete_item_without_any_reader() {
    // The point of the split: a writer can build an Item from data alone.
    let props = City3dProperties {
        version: Some("2.0".to_string()),
        lods: vec!["2.2".to_string()],
        co_types: vec!["Building".to_string()],
        ..Default::default()
    };
    let bbox = BBox3D {
        xmin: 4.3,
        ymin: 51.9,
        zmin: 0.0,
        xmax: 4.4,
        ymax: 52.0,
        zmax: 20.0,
    };

    let item = StacItemBuilder::new("no-reader-needed")
        .bbox(bbox)
        .geometry_from_bbox()
        .city3d(props)
        .unwrap()
        .crs(&CRS::from_epsg(7415))
        .build()
        .unwrap();

    assert_eq!(item.id, "no-reader-needed");
    assert_eq!(
        item.properties.additional_fields["city3d:lods"],
        serde_json::json!(["2.2"])
    );
    assert_eq!(item.properties.additional_fields["proj:code"], "EPSG:7415");
    assert!(item.geometry.is_some());
    assert!(
        item.extensions
            .contains(&city3d_stac_types::extensions::PROJECTION_EXTENSION.to_string()),
        "proj:code must pull in the projection extension"
    );
}
