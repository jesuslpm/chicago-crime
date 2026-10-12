//! Primary table key and value layout, using the FoundationDB tuple format (decision D4).

use std::borrow::Cow;
use std::fmt;

use foundationdb_tuple::{Element, pack, unpack};

use crate::{CivilDateTime, CrimeRecord, HEADER};

/// Number of elements in a primary table value: every CSV column except `Location`.
pub const VALUE_FIELD_COUNT: usize = 21;

/// Why a primary table value could not be decoded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeError {
    /// The bytes are not a well-formed tuple.
    Malformed(String),
    /// The tuple does not hold exactly [`VALUE_FIELD_COUNT`] elements.
    Arity { found: usize },
    /// The element of a field has the wrong type or an invalid value.
    InvalidField { field: &'static str },
}

/// Primary table key: the packed 1-tuple `(id,)`. Byte order equals numeric order.
pub fn primary_key(id: i64) -> Vec<u8> {
    pack(&(id,))
}

/// Primary table value: one flat tuple with the 21 stored fields in CSV order, ID first,
/// `Location` excluded and `None` packed as `Nil`.
pub fn encode_value(record: &CrimeRecord) -> Vec<u8> {
    let fields: [Element<'_>; VALUE_FIELD_COUNT] = [
        Element::Int(record.id),
        text(&record.case_number),
        Element::Int(record.date.seconds()),
        text(&record.block),
        text(&record.iucr),
        text(&record.primary_type),
        text(&record.description),
        optional(record.location_description.as_deref(), text),
        Element::Bool(record.arrest),
        Element::Bool(record.domestic),
        text(&record.beat),
        optional(record.district.as_deref(), text),
        optional(record.ward, Element::Int),
        optional(record.community_area, Element::Int),
        text(&record.fbi_code),
        optional(record.x_coordinate, Element::Int),
        optional(record.y_coordinate, Element::Int),
        Element::Int(record.year),
        Element::Int(record.updated_on.seconds()),
        optional(record.latitude, Element::Double),
        optional(record.longitude, Element::Double),
    ];
    // A root-level slice packs as one flat tuple, not as a nested one.
    pack(&fields.as_slice())
}

/// Decodes a value written by [`encode_value`], rejecting any other arity or element type.
pub fn decode_value(bytes: &[u8]) -> Result<CrimeRecord, DecodeError> {
    let elements: Vec<Element<'_>> =
        unpack(bytes).map_err(|error| DecodeError::Malformed(error.to_string()))?;
    if elements.len() != VALUE_FIELD_COUNT {
        return Err(DecodeError::Arity {
            found: elements.len(),
        });
    }
    let mut fields = FieldReader {
        elements: elements.into_iter(),
        position: 0,
    };
    // Struct fields are evaluated in the order written, which is the CSV column order.
    Ok(CrimeRecord {
        id: fields.int()?,
        case_number: fields.text()?,
        date: fields.datetime()?,
        block: fields.text()?,
        iucr: fields.text()?,
        primary_type: fields.text()?,
        description: fields.text()?,
        location_description: fields.optional_text()?,
        arrest: fields.bool()?,
        domestic: fields.bool()?,
        beat: fields.text()?,
        district: fields.optional_text()?,
        ward: fields.optional_int()?,
        community_area: fields.optional_int()?,
        fbi_code: fields.text()?,
        x_coordinate: fields.optional_int()?,
        y_coordinate: fields.optional_int()?,
        year: fields.int()?,
        updated_on: fields.datetime()?,
        latitude: fields.optional_double()?,
        longitude: fields.optional_double()?,
    })
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DecodeError::Malformed(reason) => write!(f, "the value is not a valid tuple: {reason}"),
            DecodeError::Arity { found } => write!(
                f,
                "the value holds {found} fields instead of {VALUE_FIELD_COUNT}"
            ),
            DecodeError::InvalidField { field } => {
                write!(f, "the field \"{field}\" has an unexpected type or value")
            }
        }
    }
}

impl std::error::Error for DecodeError {}

fn text(value: &str) -> Element<'_> {
    Element::String(Cow::Borrowed(value))
}

fn optional<'a, T>(value: Option<T>, element: impl FnOnce(T) -> Element<'a>) -> Element<'a> {
    value.map_or(Element::Nil, element)
}

/// Reads the decoded elements one field at a time, naming the field in every error.
struct FieldReader<'a> {
    elements: std::vec::IntoIter<Element<'a>>,
    position: usize,
}

impl<'a> FieldReader<'a> {
    /// Takes the next element and converts it, or fails naming the current field.
    fn next<T>(
        &mut self,
        convert: impl FnOnce(Element<'a>) -> Option<T>,
    ) -> Result<T, DecodeError> {
        let field = HEADER[self.position];
        self.position += 1;
        self.elements
            .next()
            .and_then(convert)
            .ok_or(DecodeError::InvalidField { field })
    }

    fn int(&mut self) -> Result<i64, DecodeError> {
        self.next(|element| match element {
            Element::Int(value) => Some(value),
            _ => None,
        })
    }

    fn optional_int(&mut self) -> Result<Option<i64>, DecodeError> {
        self.next(|element| match element {
            Element::Nil => Some(None),
            Element::Int(value) => Some(Some(value)),
            _ => None,
        })
    }

    fn text(&mut self) -> Result<String, DecodeError> {
        self.next(|element| match element {
            Element::String(value) => Some(value.into_owned()),
            _ => None,
        })
    }

    fn optional_text(&mut self) -> Result<Option<String>, DecodeError> {
        self.next(|element| match element {
            Element::Nil => Some(None),
            Element::String(value) => Some(Some(value.into_owned())),
            _ => None,
        })
    }

    fn bool(&mut self) -> Result<bool, DecodeError> {
        self.next(|element| match element {
            Element::Bool(value) => Some(value),
            _ => None,
        })
    }

    fn optional_double(&mut self) -> Result<Option<f64>, DecodeError> {
        self.next(|element| match element {
            Element::Nil => Some(None),
            Element::Double(value) => Some(Some(value)),
            _ => None,
        })
    }

    fn datetime(&mut self) -> Result<CivilDateTime, DecodeError> {
        self.next(|element| match element {
            Element::Int(seconds) => CivilDateTime::from_seconds(seconds).ok(),
            _ => None,
        })
    }
}
