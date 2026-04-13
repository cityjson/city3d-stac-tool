//! CityGML reader tests

use city3d_stac::reader::CityGMLReader;
use city3d_stac::reader::CityModelMetadataReader;
use std::path::Path;

#[test]
fn test_citygml2_reader_creation() {
    let reader = CityGMLReader::new(Path::new("tests/data/3dbag_citygml2.gml"));
    assert!(reader.is_ok());
}

#[test]
fn test_citygml3_reader_creation() {
    let reader = CityGMLReader::new(Path::new("tests/data/3dbag_citygml3.gml"));
    assert!(reader.is_ok());
}

#[test]
fn test_citygml2_version() {
    let reader = CityGMLReader::new(Path::new("tests/data/3dbag_citygml2.gml")).unwrap();
    assert_eq!(reader.version().unwrap(), "2.0");
}

#[test]
fn test_citygml3_version() {
    let reader = CityGMLReader::new(Path::new("tests/data/3dbag_citygml3.gml")).unwrap();
    assert_eq!(reader.version().unwrap(), "3.0");
}

#[test]
fn test_citygml_encoding() {
    let reader = CityGMLReader::new(Path::new("tests/data/3dbag_citygml2.gml")).unwrap();
    assert_eq!(reader.encoding(), "CityGML");
}

#[test]
fn test_citygml2_bbox_extraction() {
    let reader = CityGMLReader::new(Path::new("tests/data/3dbag_citygml2.gml")).unwrap();
    let bbox = reader.bbox().unwrap();

    // Expected values from the file header
    assert!((bbox.xmin - 84501.554688).abs() < 0.001);
    assert!((bbox.ymin - 445805.03125).abs() < 0.001);
    assert!((bbox.zmin - (-2.462002)).abs() < 0.001);
    assert!((bbox.xmax - 85675.234375).abs() < 0.001);
    assert!((bbox.ymax - 446983.46875).abs() < 0.001);
    assert!((bbox.zmax - 94.801003).abs() < 0.001);
}

#[test]
fn test_citygml3_bbox_extraction() {
    let reader = CityGMLReader::new(Path::new("tests/data/3dbag_citygml3.gml")).unwrap();
    let bbox = reader.bbox().unwrap();

    // Expected values from the file header (CityGML 3.0)
    assert!((bbox.xmin - 84501.5546875).abs() < 0.001);
    assert!((bbox.ymin - 445805.03125).abs() < 0.001);
    assert!((bbox.zmax - 94.8010025024414).abs() < 0.001);
}

#[test]
fn test_citygml2_crs_extraction() {
    let reader = CityGMLReader::new(Path::new("tests/data/3dbag_citygml2.gml")).unwrap();
    let crs = reader.crs().unwrap();
    assert_eq!(crs.epsg, Some(7415));
}

#[test]
fn test_citygml3_crs_extraction() {
    let reader = CityGMLReader::new(Path::new("tests/data/3dbag_citygml3.gml")).unwrap();
    let crs = reader.crs().unwrap();
    assert_eq!(crs.epsg, Some(7415));
}

#[test]
fn test_citygml2_city_object_count() {
    let reader = CityGMLReader::new(Path::new("tests/data/3dbag_citygml2.gml")).unwrap();
    let count = reader.city_object_count().unwrap();
    assert_eq!(
        count, 1110,
        "CityGML 2.0 file should have exactly 1110 city objects"
    );
}

#[test]
fn test_citygml3_city_object_count() {
    let reader = CityGMLReader::new(Path::new("tests/data/3dbag_citygml3.gml")).unwrap();
    let count = reader.city_object_count().unwrap();
    assert_eq!(
        count, 1110,
        "CityGML 3.0 file should have exactly 1110 city objects"
    );
}

#[test]
fn test_citygml_prefixed_namespace_city_object_count() {
    // CityGML file where the core namespace uses an explicit prefix (core:cityObjectMember)
    // instead of being the default namespace. This is valid and common in the wild.
    let reader = CityGMLReader::new(Path::new("tests/data/prefixed_citygml2.gml")).unwrap();
    let count = reader.city_object_count().unwrap();
    assert_eq!(
        count, 3,
        "Prefixed CityGML file should count all 3 core:cityObjectMember elements"
    );
}

#[test]
fn test_citygml_prefixed_namespace_version() {
    // Verify version detection works when CityModel has a namespace prefix (core:CityModel)
    let reader = CityGMLReader::new(Path::new("tests/data/prefixed_citygml2.gml")).unwrap();
    assert_eq!(reader.version().unwrap(), "2.0");
}

#[test]
fn test_citygml_prefixed_namespace_object_types() {
    let reader = CityGMLReader::new(Path::new("tests/data/prefixed_citygml2.gml")).unwrap();
    let types = reader.city_object_types().unwrap();
    assert!(types.contains(&"Building".to_string()));
}

#[test]
fn test_citygml_prefixed_namespace_bbox() {
    let reader = CityGMLReader::new(Path::new("tests/data/prefixed_citygml2.gml")).unwrap();
    let bbox = reader.bbox().unwrap();
    assert!((bbox.xmin - 84501.554).abs() < 0.01);
    assert!((bbox.ymin - 445805.031).abs() < 0.01);
}

#[test]
fn test_citygml_prefixed_namespace_crs() {
    let reader = CityGMLReader::new(Path::new("tests/data/prefixed_citygml2.gml")).unwrap();
    let crs = reader.crs().unwrap();
    assert_eq!(crs.epsg, Some(7415));
}

#[test]
fn test_citygml_prefixed_namespace_lods() {
    let reader = CityGMLReader::new(Path::new("tests/data/prefixed_citygml2.gml")).unwrap();
    let lods = reader.lods().unwrap();
    assert!(lods.contains(&"1".to_string()), "Should detect LOD 1");
    assert!(lods.contains(&"2".to_string()), "Should detect LOD 2");
}

#[test]
fn test_citygml_prefixed_namespace_attributes() {
    let reader = CityGMLReader::new(Path::new("tests/data/prefixed_citygml2.gml")).unwrap();
    let attrs = reader.attributes().unwrap();
    let attr_names: Vec<&str> = attrs.iter().map(|a| a.name.as_str()).collect();
    assert!(
        attr_names.contains(&"b3_dak_type"),
        "Should detect b3_dak_type attribute"
    );
}

#[test]
fn test_citygml2_city_object_types() {
    let reader = CityGMLReader::new(Path::new("tests/data/3dbag_citygml2.gml")).unwrap();
    let types = reader.city_object_types().unwrap();
    assert!(
        !types.is_empty(),
        "Should have at least one city object type"
    );
    // 3DBAG datasets contain Building objects
    assert!(types.contains(&"Building".to_string()));
}

#[test]
fn test_citygml3_city_object_types() {
    let reader = CityGMLReader::new(Path::new("tests/data/3dbag_citygml3.gml")).unwrap();
    let types = reader.city_object_types().unwrap();
    assert!(
        !types.is_empty(),
        "Should have at least one city object type"
    );
    // 3DBAG datasets contain Building objects
    assert!(types.contains(&"Building".to_string()));
}

#[test]
fn test_citygml2_attributes() {
    let reader = CityGMLReader::new(Path::new("tests/data/3dbag_citygml2.gml")).unwrap();
    let attrs = reader.attributes().unwrap();
    // Check for expected 3DBAG attributes
    let attr_names: Vec<&str> = attrs.iter().map(|a| a.name.as_str()).collect();

    // Some expected 3DBAG attributes
    assert!(
        attr_names.contains(&"b3_dak_type"),
        "Should have b3_dak_type attribute"
    );
    assert!(
        attr_names.contains(&"b3_h_maaiveld"),
        "Should have b3_h_maaiveld attribute"
    );
}

#[test]
fn test_citygml3_attributes() {
    let reader = CityGMLReader::new(Path::new("tests/data/3dbag_citygml3.gml")).unwrap();
    let attrs = reader.attributes().unwrap();
    // Check for expected 3DBAG attributes
    let attr_names: Vec<&str> = attrs.iter().map(|a| a.name.as_str()).collect();

    // Some expected 3DBAG attributes (3.0 format uses different attribute naming)
    assert!(
        attr_names.contains(&"b3_dak_type"),
        "Should have b3_dak_type attribute"
    );
    assert!(
        attr_names.contains(&"b3_h_maaiveld"),
        "Should have b3_h_maaiveld attribute"
    );
}

#[test]
fn test_citygml_file_path() {
    let reader = CityGMLReader::new(Path::new("tests/data/3dbag_citygml2.gml")).unwrap();
    assert_eq!(
        reader.file_path(),
        Path::new("tests/data/3dbag_citygml2.gml")
    );
}

#[test]
fn test_citygml_not_found() {
    let reader = CityGMLReader::new(Path::new("tests/data/nonexistent.gml"));
    assert!(reader.is_err());
}
