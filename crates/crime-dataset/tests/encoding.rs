mod common;

use std::borrow::Cow;

use common::{sample_record, sparse_record};
use crime_dataset::{
    CrimeRecord, DecodeError, VALUE_FIELD_COUNT, decode_value, encode_value, primary_key,
};
use foundationdb_tuple::{Element, pack, unpack};

/// Re-encodes the sample value after replacing one element, to build invalid values.
fn sample_value_with(position: usize, element: Element<'static>) -> Vec<u8> {
    let encoded = encode_value(&sample_record());
    let mut elements: Vec<Element> = unpack(&encoded).expect("sample value is a tuple");
    elements[position] = element;
    pack(&elements)
}

#[test]
fn value_round_trips_the_sample_row() {
    let record = sample_record();

    assert_eq!(decode_value(&encode_value(&record)), Ok(record));
}

#[test]
fn value_round_trips_a_record_with_every_optional_field_empty() {
    let record = sparse_record(634);

    assert_eq!(decode_value(&encode_value(&record)), Ok(record));
}

#[test]
fn value_is_one_flat_tuple_with_nil_for_empty_fields() {
    let record = sparse_record(634);
    let encoded = encode_value(&record);
    let elements: Vec<Element> = unpack(&encoded).expect("value is a tuple");

    assert_eq!(elements.len(), VALUE_FIELD_COUNT);
    assert_eq!(elements[0], Element::Int(634));
    assert_eq!(elements[1], Element::String(Cow::Borrowed("HZ100001")));
    assert_eq!(elements[2], Element::Int(record.date.seconds()));
    assert_eq!(elements[8], Element::Bool(true));
    for empty_position in [7, 11, 12, 13, 15, 16, 19, 20] {
        assert_eq!(elements[empty_position], Element::Nil, "{empty_position}");
    }
    assert!(encoded.starts_with(&primary_key(634)), "ID comes first");
}

#[test]
fn leading_zeros_in_codes_survive_the_round_trip() {
    let sample = decode_value(&encode_value(&sample_record())).expect("decodes");
    let sparse = decode_value(&encode_value(&sparse_record(1))).expect("decodes");

    assert_eq!(sample.beat, "0314");
    assert_eq!(sample.district.as_deref(), Some("003"));
    assert_eq!(sample.fbi_code, "09");
    assert_eq!(sparse.iucr, "0110");
    assert_eq!(sparse.fbi_code, "08B");
}

#[test]
fn location_rebuilds_the_source_text_only_with_both_coordinates() {
    let mut record = sample_record();
    assert_eq!(
        record.location().as_deref(),
        Some("(41.777750747, -87.600396201)")
    );

    record.longitude = None;
    assert_eq!(record.location(), None);
    assert_eq!(sparse_record(1).location(), None);
}

#[test]
fn primary_key_byte_order_equals_numeric_order() {
    let ids = [2, 10, 100, 634, 255, 256, 65_535, 65_536, 14_353_993];
    let mut numeric = ids.to_vec();
    numeric.sort();
    let mut by_key = ids.to_vec();
    by_key.sort_by_key(|&id| primary_key(id));

    assert_eq!(by_key, numeric);
}

#[test]
fn primary_key_is_the_packed_one_tuple() {
    // FoundationDB tuple layer: code 0x14 + byte length, then big-endian bytes (634 = 0x027A).
    assert_eq!(primary_key(634), vec![0x16, 0x02, 0x7A]);
    assert_eq!(primary_key(14_348_063), pack(&(14_348_063_i64,)));
}

#[test]
fn decode_rejects_bytes_that_are_not_a_tuple() {
    for garbage in [&[0xFF, 0x13, 0x37][..], &[0x02, b'a'], &[0x16, 0x02]] {
        assert!(
            matches!(decode_value(garbage), Err(DecodeError::Malformed(_))),
            "{garbage:?}"
        );
    }
}

#[test]
fn decode_rejects_a_tuple_with_another_arity() {
    let mut too_long = encode_value(&sample_record());
    too_long.extend(pack(&(1_i64,)));

    assert_eq!(decode_value(&[]), Err(DecodeError::Arity { found: 0 }));
    assert_eq!(
        decode_value(&pack(&(1_i64, "JK1"))),
        Err(DecodeError::Arity { found: 2 })
    );
    assert_eq!(
        decode_value(&too_long),
        Err(DecodeError::Arity { found: 22 })
    );
}

#[test]
fn decode_rejects_elements_of_the_wrong_type() {
    let cases: [(usize, Element<'static>, &str); 7] = [
        (0, Element::String(Cow::Borrowed("14348063")), "ID"),
        (1, Element::Nil, "Case Number"),
        (2, Element::Int(i64::MAX), "Date"),
        (8, Element::Int(0), "Arrest"),
        (11, Element::Int(3), "District"),
        (19, Element::Float(41.7), "Latitude"),
        (
            20,
            Element::Tuple(vec![Element::Double(-87.6)]),
            "Longitude",
        ),
    ];

    for (position, element, field) in cases {
        assert_eq!(
            decode_value(&sample_value_with(position, element)),
            Err(DecodeError::InvalidField { field }),
            "{field}"
        );
    }
}

#[test]
fn decoded_record_keeps_every_field_of_the_sample_row() {
    let decoded: CrimeRecord = decode_value(&encode_value(&sample_record())).expect("decodes");

    assert_eq!(decoded.id, 14_348_063);
    assert_eq!(decoded.date.to_string(), "10/03/2026 12:00:00 AM");
    assert_eq!(decoded.updated_on.to_string(), "10/10/2026 03:42:19 PM");
    assert_eq!(decoded.ward, Some(20));
    assert_eq!(decoded.community_area, Some(42));
    assert_eq!(decoded.x_coordinate, Some(1_184_164));
    assert_eq!(decoded.y_coordinate, Some(1_862_455));
    assert_eq!(decoded.latitude, Some(41.777750747));
    assert_eq!(decoded.longitude, Some(-87.600396201));
}
