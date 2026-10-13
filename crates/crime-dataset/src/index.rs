//! The fixed set of secondary indexes and their key layout (decision D2).

use foundationdb_tuple::pack;

use crate::CrimeRecord;

/// One of the five secondary indexes of the dataset.
///
/// The discriminant is the stable ordinal; never renumber or reorder existing variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum IndexKind {
    Date = 0,
    PrimaryType = 1,
    District = 2,
    CommunityArea = 3,
    CaseNumber = 4,
}

impl IndexKind {
    /// Every index, in ordinal order.
    pub const ALL: [IndexKind; 5] = [
        IndexKind::Date,
        IndexKind::PrimaryType,
        IndexKind::District,
        IndexKind::CommunityArea,
        IndexKind::CaseNumber,
    ];

    /// Stable name used in the manifest.
    pub fn name(self) -> &'static str {
        match self {
            IndexKind::Date => "date",
            IndexKind::PrimaryType => "primary_type",
            IndexKind::District => "district",
            IndexKind::CommunityArea => "community_area",
            IndexKind::CaseNumber => "case_number",
        }
    }

    /// Stable file name of the index table inside the dataset folder:
    /// `{INDEX_FILE_PREFIX}{name}{TABLE_FILE_EXTENSION}`.
    pub fn file_name(self) -> &'static str {
        match self {
            IndexKind::Date => "index-date.sst",
            IndexKind::PrimaryType => "index-primary_type.sst",
            IndexKind::District => "index-district.sst",
            IndexKind::CommunityArea => "index-community_area.sst",
            IndexKind::CaseNumber => "index-case_number.sst",
        }
    }

    /// Stable small number that orders the indexes, used to sort entries of all indexes together.
    pub fn ordinal(self) -> u8 {
        self as u8
    }
}

/// The index form of a text value (decision C1): the text in lowercase.
///
/// The generator stores the primary type, district and case number indexes in this form, and the
/// query app converts every searched value with this same function before it uses an index, so
/// that `theft`, `Theft` and `THEFT` find the same entries. Lowercasing never pads or strips
/// anything: codes keep their leading zeros (`008` stays `008`, `8` stays `8`). The primary table
/// is not affected: it keeps every field exactly as the CSV has it.
pub fn index_text(value: &str) -> String {
    value.to_lowercase()
}

/// Index table key: the packed tuple `(value, id)`, with `Nil` as the value when the field is empty.
///
/// Text values (primary type, district and case number) are stored in their [`index_text`] form,
/// lowercase (decision C1); dates and community areas are stored as their numbers. `Nil` sorts
/// before every value, and entries that share a value sort by ascending ID. Index entries have
/// an empty value; the key carries everything.
pub fn index_key(kind: IndexKind, record: &CrimeRecord) -> Vec<u8> {
    let id = record.id;
    match kind {
        IndexKind::Date => pack(&(record.date.seconds(), id)),
        IndexKind::PrimaryType => pack(&(index_text(&record.primary_type), id)),
        IndexKind::District => pack(&(record.district.as_deref().map(index_text), id)),
        IndexKind::CommunityArea => pack(&(record.community_area, id)),
        IndexKind::CaseNumber => pack(&(index_text(&record.case_number), id)),
    }
}
