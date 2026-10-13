//! The five secondary indexes of the parent PRD (decision D2), rebuilt from the CSV alone.
//!
//! Every index table holds one entry per accepted record, with the key `pack((value, id))` and an
//! empty value; an empty CSV field is indexed as `Nil`, which sorts before every value. Text
//! values are indexed in lowercase (query app decision C1, schema version 2), lowercased here
//! with the standard library alone. A table is correct when its keys are exactly the keys rebuilt
//! here from the CSV rows, sorted byte by byte as the LevelDB table format orders them.

use std::collections::BTreeMap;

use foundationdb_tuple::{Element, pack, unpack};

use crate::civil;

/// How the indexed value of a key is built from the text of its CSV column.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyValue {
    /// Seconds of the naive civil clock of a date column.
    Date,
    /// The text in lowercase; the column is mandatory.
    Text,
    /// The text in lowercase, or `Nil` when the field is empty.
    OptionalText,
    /// The integer, or `Nil` when the field is empty.
    OptionalInt,
}

/// One secondary index.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IndexSpec {
    /// Name of the index in the manifest.
    pub name: &'static str,
    /// File of the index table inside the version folder.
    pub file: &'static str,
    /// The indexed CSV column, by position.
    pub column: usize,
    pub value: KeyValue,
}

/// The indexes of the dataset, in manifest order.
pub const INDEXES: [IndexSpec; 5] = [
    IndexSpec {
        name: "date",
        file: "index-date.sst",
        column: 2,
        value: KeyValue::Date,
    },
    IndexSpec {
        name: "primary_type",
        file: "index-primary_type.sst",
        column: 5,
        value: KeyValue::Text,
    },
    IndexSpec {
        name: "district",
        file: "index-district.sst",
        column: 11,
        value: KeyValue::OptionalText,
    },
    IndexSpec {
        name: "community_area",
        file: "index-community_area.sst",
        column: 13,
        value: KeyValue::OptionalInt,
    },
    IndexSpec {
        name: "case_number",
        file: "index-case_number.sst",
        column: 1,
        value: KeyValue::Text,
    },
];

/// The key that the index `spec` must hold for the CSV row `fields` stored under `id`, or `None`
/// when the indexed field cannot be typed (the generator would have rejected the row).
pub fn expected_key(spec: &IndexSpec, fields: &[&str], id: i64) -> Option<Vec<u8>> {
    let text = *fields.get(spec.column)?;
    let optional = (!text.is_empty()).then_some(text);
    Some(match spec.value {
        KeyValue::Date => pack(&(civil::parse_civil(text)?, id)),
        KeyValue::Text => pack(&(text.to_lowercase(), id)),
        KeyValue::OptionalText => pack(&(optional.map(str::to_lowercase), id)),
        KeyValue::OptionalInt => {
            let value = optional.map(str::parse::<i64>).transpose().ok()?;
            pack(&(value, id))
        }
    })
}

impl KeyValue {
    /// Whether the indexed values are text, which the index holds in lowercase.
    pub fn is_text(self) -> bool {
        matches!(self, KeyValue::Text | KeyValue::OptionalText)
    }
}

/// The indexed value and the ID of an index key, or `None` unless the key is a tuple of exactly
/// two elements whose last one is an integer.
pub fn decode_key(key: &[u8]) -> Option<(Element<'_>, i64)> {
    let mut elements: Vec<Element<'_>> = unpack(key).ok()?;
    let Some(Element::Int(id)) = elements.pop() else {
        return None;
    };
    let value = elements.pop()?;
    elements.is_empty().then_some((value, id))
}

/// The case numbers that more than one ID shares, with every ID that shares each, from case number
/// index keys sorted by key: entries of one case number are next to each other, IDs ascending.
pub fn shared_case_numbers<'a>(
    keys: impl IntoIterator<Item = &'a [u8]>,
) -> Result<BTreeMap<String, Vec<i64>>, String> {
    let mut shared = BTreeMap::new();
    let mut group: Option<(String, Vec<i64>)> = None;
    for key in keys {
        let Some((Element::String(case_number), id)) = decode_key(key) else {
            return Err(format!("{key:02x?} is not a (case number, id) key"));
        };
        match &mut group {
            Some((current, ids)) if *current == case_number => ids.push(id),
            _ => {
                if let Some((current, ids)) = group.replace((case_number.into_owned(), vec![id]))
                    && ids.len() > 1
                {
                    shared.insert(current, ids);
                }
            }
        }
    }
    if let Some((current, ids)) = group.filter(|(_, ids)| ids.len() > 1) {
        shared.insert(current, ids);
    }
    Ok(shared)
}

#[cfg(test)]
mod tests {
    use foundationdb_tuple::pack;

    use super::*;

    fn spec(name: &str) -> &'static IndexSpec {
        INDEXES
            .iter()
            .find(|spec| spec.name == name)
            .unwrap_or_else(|| panic!("{name} is an index"))
    }

    /// A CSV row with only the indexed columns filled in.
    fn row<'a>(
        case_number: &'a str,
        date: &'a str,
        primary_type: &'a str,
        district: &'a str,
        community_area: &'a str,
    ) -> Vec<&'a str> {
        let mut fields = vec![""; 22];
        fields[1] = case_number;
        fields[2] = date;
        fields[5] = primary_type;
        fields[11] = district;
        fields[13] = community_area;
        fields
    }

    #[test]
    fn each_index_reads_the_column_the_prd_names() {
        let columns: Vec<(&str, &str)> = INDEXES
            .iter()
            .map(|spec| (spec.name, crate::fields::HEADER[spec.column]))
            .collect();

        assert_eq!(
            columns,
            [
                ("date", "Date"),
                ("primary_type", "Primary Type"),
                ("district", "District"),
                ("community_area", "Community Area"),
                ("case_number", "Case Number"),
            ]
        );
        for spec in &INDEXES {
            assert_eq!(spec.file, format!("index-{}.sst", spec.name));
        }
    }

    #[test]
    fn keys_pack_the_indexed_value_and_the_id_with_text_values_in_lowercase() {
        let fields = row("JK438266", "10/03/2026 12:00:00 AM", "ARSON", "003", "42");
        let id = 14_348_063;

        assert_eq!(
            expected_key(spec("date"), &fields, id),
            Some(pack(&(1_790_985_600_i64, id)))
        );
        assert_eq!(
            expected_key(spec("primary_type"), &fields, id),
            Some(pack(&("arson", id)))
        );
        assert_eq!(
            expected_key(spec("district"), &fields, id),
            Some(pack(&("003", id)))
        );
        assert_eq!(
            expected_key(spec("community_area"), &fields, id),
            Some(pack(&(42_i64, id)))
        );
        assert_eq!(
            expected_key(spec("case_number"), &fields, id),
            Some(pack(&("jk438266", id)))
        );
    }

    #[test]
    fn mixed_case_text_is_expected_in_lowercase_and_codes_keep_their_zeros() {
        let fields = row("jA123", "10/03/2026 12:00:00 AM", "Theft", "01A", "8");
        let id = 7;

        assert_eq!(
            expected_key(spec("primary_type"), &fields, id),
            Some(pack(&("theft", id)))
        );
        assert_eq!(
            expected_key(spec("district"), &fields, id),
            Some(pack(&("01a", id)))
        );
        assert_eq!(
            expected_key(spec("case_number"), &fields, id),
            Some(pack(&("ja123", id)))
        );
        let zeros = row("JC1", "10/03/2026 12:00:00 AM", "THEFT", "008", "8");
        assert_eq!(
            expected_key(spec("district"), &zeros, id),
            Some(pack(&("008", id)))
        );
    }

    #[test]
    fn empty_optional_fields_are_nil_and_sort_first() {
        let empty = row("JC1", "01/01/2001 12:00:00 AM", "THEFT", "", "");
        let filled = row("JC1", "01/01/2001 12:00:00 AM", "THEFT", "001", "1");

        for name in ["district", "community_area"] {
            let nil = expected_key(spec(name), &empty, 9).expect("Nil is a value");
            let value = expected_key(spec(name), &filled, 1).expect("a value");
            assert_eq!(nil, pack(&(None::<i64>, 9_i64)), "{name}");
            assert!(nil < value, "{name}: Nil sorts before every value");
            let (element, id) = decode_key(&nil).expect("a key decodes");
            assert!(matches!(element, Element::Nil), "{name}");
            assert_eq!(id, 9);
        }
    }

    #[test]
    fn a_field_that_cannot_be_typed_has_no_key() {
        let fields = row("JC1", "02/30/2001 12:00:00 AM", "THEFT", "", "4x");

        assert_eq!(expected_key(spec("date"), &fields, 1), None);
        assert_eq!(expected_key(spec("community_area"), &fields, 1), None);
    }

    #[test]
    fn a_key_that_is_not_a_value_and_an_id_does_not_decode() {
        assert!(decode_key(&pack(&("THEFT",))).is_none());
        assert!(decode_key(&pack(&("THEFT", "1"))).is_none());
        assert!(decode_key(&pack(&("THEFT", 1_i64, 2_i64))).is_none());
        assert!(decode_key(&[0xFF]).is_none());
    }

    #[test]
    fn shared_case_numbers_list_every_id_that_shares_them() {
        let keys = [
            pack(&("JA1", 5_i64)),
            pack(&("JA2", 1_i64)),
            pack(&("JA2", 7_i64)),
            pack(&("JA2", 9_i64)),
            pack(&("JA3", 2_i64)),
            pack(&("JA4", 3_i64)),
            pack(&("JA4", 4_i64)),
        ];

        let shared = shared_case_numbers(keys.iter().map(Vec::as_slice)).expect("valid keys");

        assert_eq!(
            shared,
            BTreeMap::from([
                ("JA2".to_owned(), vec![1, 7, 9]),
                ("JA4".to_owned(), vec![3, 4]),
            ])
        );
        let broken = [pack(&(1_i64, 2_i64))];
        assert!(shared_case_numbers(broken.iter().map(Vec::as_slice)).is_err());
    }
}
