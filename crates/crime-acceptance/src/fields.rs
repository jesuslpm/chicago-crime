//! The CSV columns and the field rules of the parent PRD, written here again on purpose: the
//! verifier checks the generator against the PRD, not against the generator's own contract.
//!
//! A value of the primary table is one flat FoundationDB tuple with the 21 stored fields in CSV
//! order: `ID` first, `Location` excluded, `Nil` for an empty optional field. A stored field is
//! equal to its CSV field when it formats back to exactly the published text: codes are text with
//! their leading zeros, booleans are `true`/`false`, integers are plain decimal numbers, decimals
//! use the shortest text that round-trips, and dates use `MM/DD/YYYY hh:mm:ss AM|PM`.

use foundationdb_tuple::{Element, unpack};

use crate::civil;

/// Columns of the source CSV, in file order.
pub const HEADER: [&str; 22] = [
    "ID",
    "Case Number",
    "Date",
    "Block",
    "IUCR",
    "Primary Type",
    "Description",
    "Location Description",
    "Arrest",
    "Domestic",
    "Beat",
    "District",
    "Ward",
    "Community Area",
    "FBI Code",
    "X Coordinate",
    "Y Coordinate",
    "Year",
    "Updated On",
    "Latitude",
    "Longitude",
    "Location",
];

/// Fields of a primary table value: every column but the last one, `Location`.
pub const VALUE_FIELDS: usize = 21;

/// How a stored field relates to the text of its CSV column.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldKind {
    /// An integer: `Int`.
    Int,
    /// Text or a code, leading zeros included: `String`.
    Text,
    /// Seconds of the naive civil clock: `Int`.
    Date,
    /// `true` or `false`: `Bool`.
    Bool,
    /// Optional text: `Nil` when the CSV field is empty, `String` otherwise.
    OptionalText,
    /// Optional integer: `Nil` or `Int`.
    OptionalInt,
    /// Optional decimal: `Nil` or `Double`.
    OptionalDouble,
}

/// The kind of every stored field, in CSV order (parent PRD, "Reglas de los campos").
pub const FIELD_KINDS: [FieldKind; VALUE_FIELDS] = [
    FieldKind::Int,            // ID
    FieldKind::Text,           // Case Number
    FieldKind::Date,           // Date
    FieldKind::Text,           // Block
    FieldKind::Text,           // IUCR
    FieldKind::Text,           // Primary Type
    FieldKind::Text,           // Description
    FieldKind::OptionalText,   // Location Description
    FieldKind::Bool,           // Arrest
    FieldKind::Bool,           // Domestic
    FieldKind::Text,           // Beat
    FieldKind::OptionalText,   // District
    FieldKind::OptionalInt,    // Ward
    FieldKind::OptionalInt,    // Community Area
    FieldKind::Text,           // FBI Code
    FieldKind::OptionalInt,    // X Coordinate
    FieldKind::OptionalInt,    // Y Coordinate
    FieldKind::Int,            // Year
    FieldKind::Date,           // Updated On
    FieldKind::OptionalDouble, // Latitude
    FieldKind::OptionalDouble, // Longitude
];

/// A stored field that differs from its CSV field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldMismatch {
    pub column: &'static str,
    /// The CSV text.
    pub csv: String,
    /// What the table holds, described.
    pub stored: String,
}

/// The elements of a primary table value, by position: it must be one flat tuple of exactly
/// [`VALUE_FIELDS`] elements.
pub fn decode_value(bytes: &[u8]) -> Result<Vec<Element<'_>>, String> {
    let elements: Vec<Element<'_>> =
        unpack(bytes).map_err(|error| format!("the value is not a tuple: {error}"))?;
    if elements.len() == VALUE_FIELDS {
        Ok(elements)
    } else {
        Err(format!(
            "the value holds {} fields instead of {VALUE_FIELDS}",
            elements.len()
        ))
    }
}

/// Whether the stored element formats back to exactly the CSV `text` under the rule of `kind`.
pub fn compare_field(kind: FieldKind, text: &str, stored: &Element<'_>) -> bool {
    match (kind, stored) {
        (
            FieldKind::OptionalText | FieldKind::OptionalInt | FieldKind::OptionalDouble,
            Element::Nil,
        ) => text.is_empty(),
        (FieldKind::Text | FieldKind::OptionalText, Element::String(value)) => {
            // An optional field is `Nil` when empty, never an empty string.
            value == text && !(kind == FieldKind::OptionalText && text.is_empty())
        }
        (FieldKind::Int | FieldKind::OptionalInt, Element::Int(value)) => value.to_string() == text,
        (FieldKind::Date, Element::Int(seconds)) => civil::format_civil(*seconds) == text,
        (FieldKind::Bool, Element::Bool(value)) => text == if *value { "true" } else { "false" },
        // `f64` displays the shortest text that parses back to the same value.
        (FieldKind::OptionalDouble, Element::Double(value)) => value.to_string() == text,
        _ => false,
    }
}

/// Every stored field that differs from its CSV field, in column order. `fields` is the whole
/// CSV row; its last column, `Location`, is not stored and never compared.
pub fn compare_row(fields: &[&str], stored: &[Element<'_>]) -> Vec<FieldMismatch> {
    FIELD_KINDS
        .iter()
        .enumerate()
        .filter_map(|(column, &kind)| {
            let text = fields.get(column).copied().unwrap_or_default();
            let element = stored.get(column);
            let equal = element.is_some_and(|element| compare_field(kind, text, element));
            (!equal).then(|| FieldMismatch {
                column: HEADER[column],
                csv: text.to_owned(),
                stored: element
                    .map_or_else(|| "nothing".to_owned(), |element| format!("{element:?}")),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    use foundationdb_tuple::pack;

    use super::*;

    /// The first data row of the reference CSV.
    const SAMPLE_ROW: [&str; 22] = [
        "14348063",
        "JK438266",
        "10/03/2026 12:00:00 AM",
        "064XX S ELLIS AVE",
        "1090",
        "ARSON",
        "ATTEMPT ARSON",
        "VEHICLE NON-COMMERCIAL",
        "false",
        "false",
        "0314",
        "003",
        "20",
        "42",
        "09",
        "1184164",
        "1862455",
        "2026",
        "10/10/2026 03:42:19 PM",
        "41.777750747",
        "-87.600396201",
        "(41.777750747, -87.600396201)",
    ];

    /// Seconds of `10/03/2026 12:00:00 AM` and `10/10/2026 03:42:19 PM`, computed by hand.
    const DATE: i64 = 1_790_985_600;
    const UPDATED_ON: i64 = 1_791_646_939;

    fn text(value: &str) -> Element<'_> {
        Element::String(Cow::Borrowed(value))
    }

    /// [`SAMPLE_ROW`] as the generator should store it.
    fn sample_value() -> Vec<Element<'static>> {
        vec![
            Element::Int(14_348_063),
            text("JK438266"),
            Element::Int(DATE),
            text("064XX S ELLIS AVE"),
            text("1090"),
            text("ARSON"),
            text("ATTEMPT ARSON"),
            text("VEHICLE NON-COMMERCIAL"),
            Element::Bool(false),
            Element::Bool(false),
            text("0314"),
            text("003"),
            Element::Int(20),
            Element::Int(42),
            text("09"),
            Element::Int(1_184_164),
            Element::Int(1_862_455),
            Element::Int(2026),
            Element::Int(UPDATED_ON),
            Element::Double(41.777_750_747),
            Element::Double(-87.600_396_201),
        ]
    }

    #[test]
    fn a_flat_tuple_of_21_fields_decodes_in_order() {
        let bytes = pack(&sample_value().as_slice());

        let decoded = decode_value(&bytes).expect("a value of 21 fields");

        assert_eq!(decoded.len(), VALUE_FIELDS);
        assert!(compare_row(&SAMPLE_ROW, &decoded).is_empty());
    }

    #[test]
    fn a_value_with_another_arity_or_broken_bytes_is_refused() {
        let mut short = sample_value();
        short.pop();
        let error = decode_value(&pack(&short.as_slice())).expect_err("20 fields");
        assert!(error.contains("20"), "{error}");

        assert!(decode_value(&[0xFF]).is_err());
        assert!(
            decode_value(&[0x02, b'a']).is_err(),
            "an unterminated string"
        );
    }

    #[test]
    fn each_kind_compares_the_published_text() {
        use FieldKind::*;
        let equal = [
            (Int, "14348063", Element::Int(14_348_063)),
            (Int, "-5", Element::Int(-5)),
            (Text, "0314", text("0314")),
            (Text, "08B", text("08B")),
            (Date, "10/03/2026 12:00:00 AM", Element::Int(DATE)),
            (Bool, "true", Element::Bool(true)),
            (Bool, "false", Element::Bool(false)),
            (OptionalText, "", Element::Nil),
            (OptionalText, "STREET", text("STREET")),
            (OptionalInt, "", Element::Nil),
            (OptionalInt, "0", Element::Int(0)),
            (OptionalInt, "42", Element::Int(42)),
            (OptionalDouble, "", Element::Nil),
            (
                OptionalDouble,
                "41.777750747",
                Element::Double(41.777_750_747),
            ),
            (
                OptionalDouble,
                "-87.68427754",
                Element::Double(-87.684_277_54),
            ),
        ];
        for (kind, csv, stored) in &equal {
            assert!(
                compare_field(*kind, csv, stored),
                "{kind:?} {csv:?} {stored:?}"
            );
        }

        let different = [
            (Int, "14348063", Element::Int(1)),
            (Int, "14348063", text("14348063")),
            (Int, "014", Element::Int(14)),
            (Int, "", Element::Int(0)),
            (Text, "0314", text("314")),
            (Text, "0314", Element::Int(314)),
            (Text, "", Element::Nil),
            (Date, "10/03/2026 12:00:00 PM", Element::Int(DATE)),
            (
                Date,
                "10/03/2026 12:00:00 AM",
                text("10/03/2026 12:00:00 AM"),
            ),
            (Bool, "true", Element::Bool(false)),
            (Bool, "TRUE", Element::Bool(true)),
            (Bool, "true", Element::Int(1)),
            (OptionalText, "", text("")),
            (OptionalText, "STREET", Element::Nil),
            (OptionalText, "STREET", text("street")),
            (OptionalInt, "42", Element::Nil),
            (OptionalInt, "", Element::Int(0)),
            (OptionalInt, "42", text("42")),
            (OptionalDouble, "41.7", Element::Double(41.700_000_01)),
            (OptionalDouble, "41.70", Element::Double(41.7)),
            (OptionalDouble, "41.7", Element::Float(41.7)),
            (OptionalDouble, "41.7", Element::Nil),
            (OptionalDouble, "", Element::Double(0.0)),
        ];
        for (kind, csv, stored) in &different {
            assert!(
                !compare_field(*kind, csv, stored),
                "{kind:?} {csv:?} {stored:?}"
            );
        }
    }

    #[test]
    fn a_row_reports_each_column_that_differs() {
        let mut stored = sample_value();
        stored[10] = text("314");
        stored[12] = Element::Nil;

        let mismatches = compare_row(&SAMPLE_ROW, &stored);

        assert_eq!(
            mismatches,
            [
                FieldMismatch {
                    column: "Beat",
                    csv: "0314".to_owned(),
                    stored: "String(\"314\")".to_owned(),
                },
                FieldMismatch {
                    column: "Ward",
                    csv: "20".to_owned(),
                    stored: "Nil".to_owned(),
                },
            ]
        );
    }

    #[test]
    fn location_is_never_compared() {
        let mut row = SAMPLE_ROW;
        row[21] = "(0, 0)";

        assert!(compare_row(&row, &sample_value()).is_empty());
    }
}
