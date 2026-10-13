//! The field catalog: the 22 CSV columns with the business type, optionality and index of each
//! (parent PRD, "Reglas de los campos"; decisions D2, C3, C10).

use crime_dataset::{HEADER, IndexKind};

/// The business type of a field, which decides how a searched value is read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldType {
    Integer,
    /// Text and codes; codes keep their leading zeros (`0314`, `003`, `08B`).
    Text,
    /// `true`/`false` in the CSV.
    Boolean,
    Decimal,
    /// Civil date and time (decision D3).
    DateTime,
    /// `Location`, rebuilt from latitude and longitude and never stored.
    Derived,
}

/// One CSV column, in [`HEADER`] order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Field {
    Id,
    CaseNumber,
    Date,
    Block,
    Iucr,
    PrimaryType,
    Description,
    LocationDescription,
    Arrest,
    Domestic,
    Beat,
    District,
    Ward,
    CommunityArea,
    FbiCode,
    XCoordinate,
    YCoordinate,
    Year,
    UpdatedOn,
    Latitude,
    Longitude,
    Location,
}

impl Field {
    /// Every field, in CSV column order.
    pub const ALL: [Field; 22] = [
        Field::Id,
        Field::CaseNumber,
        Field::Date,
        Field::Block,
        Field::Iucr,
        Field::PrimaryType,
        Field::Description,
        Field::LocationDescription,
        Field::Arrest,
        Field::Domestic,
        Field::Beat,
        Field::District,
        Field::Ward,
        Field::CommunityArea,
        Field::FbiCode,
        Field::XCoordinate,
        Field::YCoordinate,
        Field::Year,
        Field::UpdatedOn,
        Field::Latitude,
        Field::Longitude,
        Field::Location,
    ];

    /// The CSV header name, the name the analyst types and every message shows.
    pub fn name(self) -> &'static str {
        HEADER[self as usize]
    }

    pub fn field_type(self) -> FieldType {
        match self {
            Field::Id
            | Field::Ward
            | Field::CommunityArea
            | Field::XCoordinate
            | Field::YCoordinate
            | Field::Year => FieldType::Integer,
            Field::CaseNumber
            | Field::Block
            | Field::Iucr
            | Field::PrimaryType
            | Field::Description
            | Field::LocationDescription
            | Field::Beat
            | Field::District
            | Field::FbiCode => FieldType::Text,
            Field::Arrest | Field::Domestic => FieldType::Boolean,
            Field::Latitude | Field::Longitude => FieldType::Decimal,
            Field::Date | Field::UpdatedOn => FieldType::DateTime,
            Field::Location => FieldType::Derived,
        }
    }

    /// Whether the field may be empty ("sin valor").
    pub fn is_optional(self) -> bool {
        matches!(
            self,
            Field::LocationDescription
                | Field::District
                | Field::Ward
                | Field::CommunityArea
                | Field::XCoordinate
                | Field::YCoordinate
                | Field::Latitude
                | Field::Longitude
        )
    }

    /// The secondary index of the field, if it has one. The ID has none: it is the key of the
    /// primary table.
    pub fn index(self) -> Option<IndexKind> {
        match self {
            Field::CaseNumber => Some(IndexKind::CaseNumber),
            Field::Date => Some(IndexKind::Date),
            Field::PrimaryType => Some(IndexKind::PrimaryType),
            Field::District => Some(IndexKind::District),
            Field::CommunityArea => Some(IndexKind::CommunityArea),
            _ => None,
        }
    }

    /// Whether a condition on the field is resolved through an index or the primary table
    /// (decision C10).
    pub fn is_indexed(self) -> bool {
        self == Field::Id || self.index().is_some()
    }

    /// Whether the field admits conditions: every stored field but the long texts, Block and
    /// Description (decision C3); Location is not stored.
    pub fn is_conditionable(self) -> bool {
        !matches!(self, Field::Block | Field::Description | Field::Location)
    }

    /// The conditionable fields, in CSV column order.
    pub fn conditionable() -> impl Iterator<Item = Field> {
        Self::ALL
            .into_iter()
            .filter(|field| field.is_conditionable())
    }

    /// The field named `name`, ignoring ASCII case and the spaces around it.
    pub fn find(name: &str) -> Option<Field> {
        let name = name.trim();
        Self::ALL
            .into_iter()
            .find(|field| field.name().eq_ignore_ascii_case(name))
    }
}
