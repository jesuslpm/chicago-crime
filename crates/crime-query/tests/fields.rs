//! The field catalog: every CSV column with its type, whether it is optional, indexed and
//! conditionable, and how a typed name finds it.

use crime_dataset::{HEADER, IndexKind};
use crime_query::{Field, FieldType};

#[test]
fn the_catalog_lists_every_csv_column_in_order() {
    let names: Vec<&str> = Field::ALL.iter().map(|field| field.name()).collect();
    assert_eq!(names, HEADER);
}

#[test]
fn every_field_has_the_type_of_the_parent_field_rules() {
    use FieldType::*;
    let expected = [
        ("ID", Integer),
        ("Case Number", Text),
        ("Date", DateTime),
        ("Block", Text),
        ("IUCR", Text),
        ("Primary Type", Text),
        ("Description", Text),
        ("Location Description", Text),
        ("Arrest", Boolean),
        ("Domestic", Boolean),
        ("Beat", Text),
        ("District", Text),
        ("Ward", Integer),
        ("Community Area", Integer),
        ("FBI Code", Text),
        ("X Coordinate", Integer),
        ("Y Coordinate", Integer),
        ("Year", Integer),
        ("Updated On", DateTime),
        ("Latitude", Decimal),
        ("Longitude", Decimal),
        ("Location", Derived),
    ];
    for (field, (name, field_type)) in Field::ALL.iter().zip(expected) {
        assert_eq!(field.name(), name);
        assert_eq!(field.field_type(), field_type, "{name}");
    }
}

#[test]
fn optional_fields_are_the_ones_the_parent_allows_empty() {
    let optional: Vec<&str> = Field::ALL
        .iter()
        .filter(|field| field.is_optional())
        .map(|field| field.name())
        .collect();
    assert_eq!(
        optional,
        [
            "Location Description",
            "District",
            "Ward",
            "Community Area",
            "X Coordinate",
            "Y Coordinate",
            "Latitude",
            "Longitude",
        ]
    );
}

#[test]
fn indexed_fields_are_the_id_and_the_five_secondary_indexes() {
    let indexed: Vec<(&str, Option<IndexKind>)> = Field::ALL
        .iter()
        .filter(|field| field.is_indexed())
        .map(|field| (field.name(), field.index()))
        .collect();
    assert_eq!(
        indexed,
        [
            ("ID", None),
            ("Case Number", Some(IndexKind::CaseNumber)),
            ("Date", Some(IndexKind::Date)),
            ("Primary Type", Some(IndexKind::PrimaryType)),
            ("District", Some(IndexKind::District)),
            ("Community Area", Some(IndexKind::CommunityArea)),
        ]
    );
}

#[test]
fn block_description_and_location_are_not_conditionable() {
    let excluded: Vec<&str> = Field::ALL
        .iter()
        .filter(|field| !field.is_conditionable())
        .map(|field| field.name())
        .collect();
    assert_eq!(excluded, ["Block", "Description", "Location"]);
    assert_eq!(Field::conditionable().count(), 19);
}

#[test]
fn names_are_found_case_insensitively_after_trimming_spaces() {
    for (typed, field) in [
        ("ID", Field::Id),
        ("id", Field::Id),
        ("primary type", Field::PrimaryType),
        ("  Primary Type ", Field::PrimaryType),
        ("COMMUNITY AREA", Field::CommunityArea),
        ("updated on", Field::UpdatedOn),
        ("fbi code", Field::FbiCode),
        ("block", Field::Block),
    ] {
        assert_eq!(Field::find(typed), Some(field), "{typed:?}");
    }
    for typed in ["", "Primary", "PrimaryType", "Primary  Type", "Fecha"] {
        assert_eq!(Field::find(typed), None, "{typed:?}");
    }
}
