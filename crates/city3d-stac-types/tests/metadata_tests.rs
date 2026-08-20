//! The metadata vocabulary must be usable straight from the types crate,
//! including reprojection of a compound CRS.

use city3d_stac_types::metadata::{AttributeDefinition, AttributeType, BBox3D, CRS};

#[test]
fn attribute_definition_serialises_with_pascal_case_type() {
    let def = AttributeDefinition::new("height", AttributeType::Number);
    let json = serde_json::to_value(&def).unwrap();
    assert_eq!(json["name"], "height");
    assert_eq!(json["type"], "Number");
}

#[test]
fn compound_dutch_crs_reprojects_to_wgs84() {
    // EPSG:7415 = EPSG:28992 (RD New) + EPSG:5709 (NAP height).
    // Delft city centre in RD New coordinates.
    let bbox = BBox3D {
        xmin: 84_000.0,
        ymin: 447_000.0,
        zmin: 0.0,
        xmax: 85_000.0,
        ymax: 448_000.0,
        zmax: 20.0,
    };
    let crs = CRS::from_epsg(7415);
    let wgs = bbox.to_wgs84(&crs).expect("7415 must reproject");
    assert!(
        (4.3..4.4).contains(&wgs.xmin),
        "longitude {} not near Delft",
        wgs.xmin
    );
    assert!(
        (51.9..52.1).contains(&wgs.ymin),
        "latitude {} not near Delft",
        wgs.ymin
    );
    assert_eq!(wgs.zmin, 0.0, "Z must pass through unchanged");
}

#[test]
fn unknown_crs_outside_wgs84_range_is_an_error() {
    let bbox = BBox3D {
        xmin: 84_000.0,
        ymin: 447_000.0,
        zmin: 0.0,
        xmax: 85_000.0,
        ymax: 448_000.0,
        zmax: 20.0,
    };
    let crs = CRS::unknown();
    assert!(
        bbox.to_wgs84(&crs).is_err(),
        "projected coordinates with no CRS must not be silently treated as WGS84"
    );
}
