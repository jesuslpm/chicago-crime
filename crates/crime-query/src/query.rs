//! The typed query: the raw `--where`, `--range` and `--startswith` conditions checked, typed and
//! grouped the way the engine resolves them (decision C10).
//!
//! - The ID condition is a sorted list of distinct IDs, read from the primary table.
//! - Each equality on an indexed field (case number, primary type, district, community area)
//!   holds its distinct alternatives as [`IndexValue`]s, text already in the index form
//!   (decision C1).
//! - The date condition is a set of inclusive civil intervals: the day or minute of every
//!   `Date=` alternative (OR), intersected with every range (AND), resolved per decision C2.
//! - Every other conditionable field becomes a [`Filter`] the engine applies to the records the
//!   indexes found (decision C3); its alternatives are OR.
//! - A prefix ("empieza por", startswith PRD) is one more alternative of its field, next to the
//!   field's values (decision P1): on case number, primary type or district it belongs to the
//!   field's [`Equality`] and counts as an indexed condition (decision P3); on IUCR, Location
//!   Description, Beat or FBI Code to its [`Filter`]. Prefixes are held in the [`index_text`]
//!   form, so they compare as the values do (decision C1).
//!
//! Every condition is checked in command-line order, so the first wrong one is the one reported;
//! only then is the indexed condition required.

use std::fmt;
use std::num::NonZeroU64;

use crime_dataset::{CivilDateTime, CrimeRecord, IndexKind, index_text};
use serde::Serialize;

use crate::fields::{Field, FieldType};
use crate::index::IndexValue;
use crate::messages;

/// Seconds after the first second of a day, or of a minute, to its last second.
const REST_OF_DAY: i64 = 86_399;
const REST_OF_MINUTE: i64 = 59;

/// One condition as the analyst typed it: the field name as typed and the values exactly as
/// given. An empty value is "sin valor"; an empty range end is an open end.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RawCondition {
    /// `--where CAMPO=VALOR`.
    Equal { field: String, value: String },
    /// `--range CAMPO=DESDE..HASTA`.
    Range {
        field: String,
        from: String,
        to: String,
    },
    /// `--startswith CAMPO=PREFIJO`.
    StartsWith { field: String, prefix: String },
}

impl RawCondition {
    pub fn equal(field: &str, value: &str) -> Self {
        Self::Equal {
            field: field.to_owned(),
            value: value.to_owned(),
        }
    }

    pub fn range(field: &str, from: &str, to: &str) -> Self {
        Self::Range {
            field: field.to_owned(),
            from: from.to_owned(),
            to: to.to_owned(),
        }
    }

    pub fn starts_with(field: &str, prefix: &str) -> Self {
        Self::StartsWith {
            field: field.to_owned(),
            prefix: prefix.to_owned(),
        }
    }
}

/// Why a query was rejected for its conditions (exit code 1). [`fmt::Display`] gives the
/// message for the analyst.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryError(String);

impl fmt::Display for QueryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for QueryError {}

/// An inclusive interval of civil instants, both ends to the second.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DateInterval {
    pub from: CivilDateTime,
    pub to: CivilDateTime,
}

impl DateInterval {
    pub fn contains(&self, instant: CivilDateTime) -> bool {
        self.from <= instant && instant <= self.to
    }

    fn applied(&self) -> AppliedInterval {
        AppliedInterval {
            from: self.from.to_iso_string(),
            to: self.to.to_iso_string(),
        }
    }
}

/// An equality on a field with a secondary index other than the date: the IDs of every
/// alternative value and of every alternative prefix, joined.
#[derive(Debug, Clone, PartialEq)]
pub struct Equality {
    field: Field,
    index: IndexKind,
    values: Vec<IndexValue>,
    /// Each alternative as the analyst first typed it, one per value.
    typed: Vec<String>,
    /// The distinct prefixes in first-appearance order, in the [`index_text`] form.
    prefixes: Vec<String>,
}

impl Equality {
    pub fn field(&self) -> Field {
        self.field
    }

    pub fn index(&self) -> IndexKind {
        self.index
    }

    /// The distinct alternatives in first-appearance order, in the index form.
    pub fn values(&self) -> &[IndexValue] {
        &self.values
    }

    /// Each alternative of [`Equality::values`] as the analyst first typed it, for the messages
    /// that quote it (a missing case number).
    pub fn typed(&self) -> &[String] {
        &self.typed
    }

    /// The distinct alternative prefixes in first-appearance order, lowercase; only the text
    /// fields (case number, primary type, district) have them.
    pub fn prefixes(&self) -> &[String] {
        &self.prefixes
    }

    /// Whether the record's field is any alternative value or starts with any alternative
    /// prefix, for when the engine checks this condition on the record instead of the index.
    pub fn matches(&self, record: &CrimeRecord) -> bool {
        let actual = record_value(self.field, record);
        self.values
            .iter()
            .any(|value| Value::from_index(value).accepts(&actual))
            || starts_with_any(&self.prefixes, &actual)
    }
}

/// An equality on a field without index, applied to each record the indexes found.
#[derive(Debug, Clone, PartialEq)]
pub struct Filter {
    field: Field,
    /// The distinct alternatives; text in the [`index_text`] form, dates as their day or
    /// minute, sorted and merged.
    values: Vec<Value>,
    /// The distinct alternative prefixes of a text field, in the [`index_text`] form.
    prefixes: Vec<String>,
}

impl Filter {
    pub fn field(&self) -> Field {
        self.field
    }

    /// Whether the record's field equals any alternative: text after lowercasing both sides
    /// (decision C1), "sin valor" when the field is empty, a date within the day or minute; or
    /// whether it starts with any alternative prefix.
    pub fn matches(&self, record: &CrimeRecord) -> bool {
        let actual = record_value(self.field, record);
        self.values.iter().any(|wanted| wanted.accepts(&actual))
            || starts_with_any(&self.prefixes, &actual)
    }
}

/// Whether `actual` is text that starts with one of `prefixes`; "sin valor" never does.
fn starts_with_any(prefixes: &[String], actual: &Value) -> bool {
    match actual {
        Value::Text(text) => prefixes
            .iter()
            .any(|prefix| text.starts_with(prefix.as_str())),
        _ => false,
    }
}

/// A searched value, or a record's value, typed.
#[derive(Debug, Clone, PartialEq)]
enum Value {
    /// "Sin valor".
    Nil,
    Text(String),
    Integer(i64),
    Boolean(bool),
    Decimal(f64),
    /// A searched date: its day or minute.
    Window(DateInterval),
    /// A record's date.
    Instant(CivilDateTime),
}

impl Value {
    /// A searched index value in the form a record's value is compared with.
    fn from_index(value: &IndexValue) -> Value {
        match (value.as_text(), value.as_integer()) {
            (Some(text), _) => Value::Text(text.to_owned()),
            (None, Some(integer)) => Value::Integer(integer),
            (None, None) => Value::Nil,
        }
    }

    fn accepts(&self, actual: &Value) -> bool {
        match (self, actual) {
            (Value::Window(window), Value::Instant(instant)) => window.contains(*instant),
            (wanted, actual) => wanted == actual,
        }
    }

    fn applied(&self) -> AppliedValue {
        match self {
            Value::Nil => AppliedValue::Null,
            Value::Text(text) => AppliedValue::Text(text.clone()),
            Value::Integer(value) => AppliedValue::Integer(*value),
            Value::Boolean(value) => AppliedValue::Boolean(*value),
            Value::Decimal(value) => AppliedValue::Decimal(*value),
            Value::Window(window) => AppliedValue::Text(window.from.to_iso_string()),
            Value::Instant(instant) => AppliedValue::Text(instant.to_iso_string()),
        }
    }
}

/// A validated query.
#[derive(Debug, Clone, PartialEq)]
pub struct Query {
    /// Sorted and distinct, when there is an ID condition.
    ids: Option<Vec<i64>>,
    equalities: Vec<Equality>,
    /// Sorted, disjoint and not contiguous, when there is a date condition; possibly empty.
    dates: Option<Vec<DateInterval>>,
    filters: Vec<Filter>,
    /// Every conditioned field once, in first-appearance order.
    order: Vec<Field>,
    limit: Option<NonZeroU64>,
}

impl Query {
    /// Checks and types `conditions`, in order; the first wrong one is the error.
    pub fn build(conditions: &[RawCondition]) -> Result<Query, QueryError> {
        let mut builder = Builder::default();
        for condition in conditions {
            builder.add(condition).map_err(QueryError)?;
        }
        builder.finish().map_err(QueryError)
    }

    /// The same query with at most `limit` results.
    pub fn with_limit(self, limit: Option<NonZeroU64>) -> Self {
        Self { limit, ..self }
    }

    pub fn limit(&self) -> Option<NonZeroU64> {
        self.limit
    }

    /// The requested IDs, sorted and distinct, when there is an ID condition.
    pub fn ids(&self) -> Option<&[i64]> {
        self.ids.as_deref()
    }

    /// The equalities on indexed fields other than the date, in first-appearance order.
    pub fn equalities(&self) -> &[Equality] {
        &self.equalities
    }

    /// The date condition as sorted, disjoint inclusive intervals, when there is one. An empty
    /// set is a valid condition that no record meets.
    pub fn dates(&self) -> Option<&[DateInterval]> {
        self.dates.as_deref()
    }

    /// The conditions on fields without index, in first-appearance order.
    pub fn filters(&self) -> &[Filter] {
        &self.filters
    }

    /// Whether `record` meets every filter. The indexed conditions are not checked here: the
    /// records come from the indexes.
    pub fn matches(&self, record: &CrimeRecord) -> bool {
        self.filters.iter().all(|filter| filter.matches(record))
    }

    /// The query as applied, for the JSON output (decision C9).
    pub fn interpreted(&self) -> Interpreted {
        let conditions = self
            .order
            .iter()
            .map(|&field| AppliedCondition {
                field: field.name(),
                indexed: field.is_indexed(),
                applied: self.applied(field),
                startswith: self.applied_prefixes(field),
            })
            .collect();
        Interpreted {
            conditions,
            limit: self.limit.map(NonZeroU64::get),
        }
    }

    /// The prefixes of `field`, when it has any.
    fn applied_prefixes(&self, field: Field) -> Option<Vec<String>> {
        let equality = self.equalities.iter().find(|e| e.field == field);
        let filter = self.filters.iter().find(|f| f.field == field);
        let prefixes = equality
            .map(|equality| &equality.prefixes)
            .or(filter.map(|filter| &filter.prefixes))?;
        (!prefixes.is_empty()).then(|| prefixes.clone())
    }

    fn applied(&self, field: Field) -> Applied {
        let intervals = |set: &[DateInterval]| {
            Applied::Intervals(set.iter().map(DateInterval::applied).collect())
        };
        if field == Field::Id {
            let ids = self.ids.as_deref().unwrap_or_default();
            return Applied::Values(ids.iter().map(|&id| AppliedValue::Integer(id)).collect());
        }
        if field == Field::Date {
            return intervals(self.dates.as_deref().unwrap_or_default());
        }
        if let Some(equality) = self.equalities.iter().find(|e| e.field == field) {
            return Applied::Values(equality.values.iter().map(applied_index_value).collect());
        }
        let values = self
            .filters
            .iter()
            .find(|filter| filter.field == field)
            .map_or(&[][..], |filter| &filter.values);
        if field.field_type() == FieldType::DateTime {
            let windows: Vec<DateInterval> = values
                .iter()
                .filter_map(|value| match value {
                    Value::Window(window) => Some(*window),
                    _ => None,
                })
                .collect();
            return intervals(&windows);
        }
        Applied::Values(values.iter().map(Value::applied).collect())
    }
}

/// The query as applied: every condition with its values in applied form, and the limit.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Interpreted {
    pub conditions: Vec<AppliedCondition>,
    /// `null` without a limit.
    pub limit: Option<u64>,
}

/// One condition as applied: `{"field", "indexed", "values" | "intervals", "startswith"?}`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AppliedCondition {
    /// The CSV header name.
    pub field: &'static str,
    /// Whether an index (or the primary table) resolves it, rather than a filter.
    pub indexed: bool,
    /// The values; empty when the field has only prefixes.
    #[serde(flatten)]
    pub applied: Applied,
    /// The alternative prefixes, lowercase; left out when the field has none.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub startswith: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Applied {
    /// The distinct alternatives: lowercase text, integers, booleans, decimals, `null` for
    /// "sin valor".
    Values(Vec<AppliedValue>),
    /// Inclusive civil intervals, as `YYYY-MM-DDThh:mm:ss`.
    Intervals(Vec<AppliedInterval>),
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)]
pub enum AppliedValue {
    Null,
    Boolean(bool),
    Integer(i64),
    Decimal(f64),
    Text(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AppliedInterval {
    pub from: String,
    pub to: String,
}

fn applied_index_value(value: &IndexValue) -> AppliedValue {
    if let Some(text) = value.as_text() {
        AppliedValue::Text(text.to_owned())
    } else if let Some(integer) = value.as_integer() {
        AppliedValue::Integer(integer)
    } else {
        AppliedValue::Null
    }
}

/// The conditions gathered so far.
#[derive(Default)]
struct Builder {
    order: Vec<Field>,
    ids: Vec<i64>,
    equalities: Vec<Equality>,
    /// The day or minute of every `Date=` alternative.
    date_alternatives: Vec<DateInterval>,
    date_ranges: Vec<DateInterval>,
    filters: Vec<Filter>,
}

impl Builder {
    fn add(&mut self, condition: &RawCondition) -> Result<(), String> {
        match condition {
            RawCondition::Equal { field, value } => self.add_equal(field, value),
            RawCondition::Range { field, from, to } => self.add_range(field, from, to),
            RawCondition::StartsWith { field, prefix } => self.add_starts_with(field, prefix),
        }
    }

    fn add_equal(&mut self, typed: &str, raw: &str) -> Result<(), String> {
        let field = Field::find(typed).ok_or_else(|| messages::unknown_field(typed))?;
        if !field.is_conditionable() {
            return Err(messages::field_not_conditionable(field.name()));
        }
        let value = typed_value(field, raw)?;
        self.note(field);
        match (field, field.index(), value) {
            (Field::Id, _, Value::Integer(id)) => self.ids.push(id),
            (_, Some(IndexKind::Date), Value::Window(window)) => {
                self.date_alternatives.push(window);
            }
            (_, Some(index), value) => {
                let value = match value {
                    Value::Nil => IndexValue::nil(),
                    Value::Integer(integer) => IndexValue::integer(integer),
                    Value::Text(_) => IndexValue::text(raw),
                    other => unreachable!("{field:?} has no {other:?} values"),
                };
                let equality = self.equality(field, index);
                if !equality.values.contains(&value) {
                    equality.values.push(value);
                    equality.typed.push(raw.to_owned());
                }
            }
            (_, None, value) => {
                let filter = self.filter(field);
                if !filter.values.contains(&value) {
                    filter.values.push(value);
                }
            }
        }
        Ok(())
    }

    /// `CAMPO=PREFIJO`: one more alternative of the field's equality or filter.
    fn add_starts_with(&mut self, typed: &str, raw: &str) -> Result<(), String> {
        let field = Field::find(typed).ok_or_else(|| messages::unknown_field(typed))?;
        if !field.is_conditionable() {
            return Err(messages::field_not_conditionable(field.name()));
        }
        if field.field_type() != FieldType::Text {
            return Err(messages::prefix_not_on_text());
        }
        if raw.is_empty() {
            return Err(messages::empty_prefix(field.name()));
        }
        self.note(field);
        let prefix = index_text(raw);
        let prefixes = match field.index() {
            Some(index) => &mut self.equality(field, index).prefixes,
            None => &mut self.filter(field).prefixes,
        };
        if !prefixes.contains(&prefix) {
            prefixes.push(prefix);
        }
        Ok(())
    }

    /// The equality on `field`, created empty with its first condition.
    fn equality(&mut self, field: Field, index: IndexKind) -> &mut Equality {
        let position = match self.equalities.iter().position(|e| e.field == field) {
            Some(position) => position,
            None => {
                self.equalities.push(Equality {
                    field,
                    index,
                    values: Vec::new(),
                    typed: Vec::new(),
                    prefixes: Vec::new(),
                });
                self.equalities.len() - 1
            }
        };
        &mut self.equalities[position]
    }

    /// The filter on `field`, created empty with its first condition.
    fn filter(&mut self, field: Field) -> &mut Filter {
        let position = match self.filters.iter().position(|f| f.field == field) {
            Some(position) => position,
            None => {
                self.filters.push(Filter {
                    field,
                    values: Vec::new(),
                    prefixes: Vec::new(),
                });
                self.filters.len() - 1
            }
        };
        &mut self.filters[position]
    }

    fn add_range(&mut self, typed: &str, from: &str, to: &str) -> Result<(), String> {
        let field = Field::find(typed).ok_or_else(|| messages::unknown_field(typed))?;
        if field != Field::Date {
            return Err(messages::range_not_on_date());
        }
        let from = match from {
            "" => CivilDateTime::MIN,
            text => date_window(text)?.from,
        };
        let to = match to {
            "" => CivilDateTime::MAX,
            text => date_window(text)?.to,
        };
        if from > to {
            return Err(messages::inverted_range());
        }
        self.note(field);
        self.date_ranges.push(DateInterval { from, to });
        Ok(())
    }

    /// Remembers the first appearance of `field`.
    fn note(&mut self, field: Field) {
        if !self.order.contains(&field) {
            self.order.push(field);
        }
    }

    fn finish(self) -> Result<Query, String> {
        if !self.order.iter().any(|field| field.is_indexed()) {
            return Err(messages::no_indexed_condition());
        }
        let ids = self.order.contains(&Field::Id).then(|| {
            let mut ids = self.ids;
            ids.sort_unstable();
            ids.dedup();
            ids
        });
        let dates = self
            .order
            .contains(&Field::Date)
            .then(|| resolve_dates(self.date_alternatives, &self.date_ranges));
        let filters = self
            .filters
            .into_iter()
            .map(|mut filter| {
                merge_windows(&mut filter.values);
                filter
            })
            .collect();
        Ok(Query {
            ids,
            equalities: self.equalities,
            dates,
            filters,
            order: self.order,
            limit: None,
        })
    }
}

/// The value of `raw` for `field`, typed; an empty value is "sin valor".
fn typed_value(field: Field, raw: &str) -> Result<Value, String> {
    let name = field.name();
    if raw.is_empty() {
        return if field.is_optional() {
            Ok(Value::Nil)
        } else {
            Err(messages::mandatory_field(name))
        };
    }
    match field.field_type() {
        FieldType::Integer => raw
            .parse()
            .map(Value::Integer)
            .map_err(|_| messages::expected_integer(raw, name)),
        FieldType::Text | FieldType::Derived => Ok(Value::Text(index_text(raw))),
        FieldType::Boolean => match index_text(raw).as_str() {
            "true" | "sí" | "si" => Ok(Value::Boolean(true)),
            "false" | "no" => Ok(Value::Boolean(false)),
            _ => Err(messages::expected_boolean(raw, name)),
        },
        FieldType::Decimal => match raw.parse::<f64>() {
            Ok(value) if value.is_finite() => Ok(Value::Decimal(value)),
            _ => Err(messages::expected_decimal(raw, name)),
        },
        FieldType::DateTime => date_window(raw).map(Value::Window),
    }
}

/// The whole day of `AAAA-MM-DD` or the whole minute of `AAAA-MM-DD hh:mm` (decision C2).
fn date_window(text: &str) -> Result<DateInterval, String> {
    parse_window(text).ok_or_else(|| messages::invalid_date(text))
}

fn parse_window(text: &str) -> Option<DateInterval> {
    // Byte offsets of the fixed shape: AAAA-MM-DD hh:mm
    //                                  0    5  8  11 14
    let bytes = text.as_bytes();
    let with_time = match bytes.len() {
        10 => false,
        16 => true,
        _ => return None,
    };
    let mut separators = vec![(4, b'-'), (7, b'-')];
    if with_time {
        separators.extend([(10, b' '), (13, b':')]);
    }
    if !separators.iter().all(|&(at, byte)| bytes[at] == byte) {
        return None;
    }
    let number = |start: usize, len: usize| -> Option<i64> {
        bytes[start..start + len]
            .iter()
            .try_fold(0, |value, &byte| {
                byte.is_ascii_digit()
                    .then(|| value * 10 + i64::from(byte - b'0'))
            })
    };
    let (year, month, day) = (number(0, 4)?, number(5, 2)?, number(8, 2)?);
    let (hour, minute) = if with_time {
        (number(11, 2)?, number(14, 2)?)
    } else {
        (0, 0)
    };
    let from = CivilDateTime::from_parts(year, month, day, hour, minute, 0).ok()?;
    let rest = if with_time {
        REST_OF_MINUTE
    } else {
        REST_OF_DAY
    };
    let to = CivilDateTime::from_seconds(from.seconds() + rest).ok()?;
    Some(DateInterval { from, to })
}

/// The date condition: the union of the `Date=` alternatives, if any, intersected with every
/// range.
fn resolve_dates(alternatives: Vec<DateInterval>, ranges: &[DateInterval]) -> Vec<DateInterval> {
    let mut set = (!alternatives.is_empty()).then(|| merge(alternatives));
    for range in ranges {
        set = Some(match set {
            None => vec![*range],
            Some(set) => intersect(&set, &[*range]),
        });
    }
    set.unwrap_or_default()
}

/// Sorts `intervals` and joins those that overlap or touch.
fn merge(mut intervals: Vec<DateInterval>) -> Vec<DateInterval> {
    intervals.sort_unstable_by_key(|interval| interval.from);
    let mut merged: Vec<DateInterval> = Vec::with_capacity(intervals.len());
    for interval in intervals {
        match merged.last_mut() {
            Some(last) if interval.from.seconds() <= last.to.seconds() + 1 => {
                last.to = last.to.max(interval.to);
            }
            _ => merged.push(interval),
        }
    }
    merged
}

/// The instants in both sets; each set sorted and disjoint.
fn intersect(left: &[DateInterval], right: &[DateInterval]) -> Vec<DateInterval> {
    let mut result = Vec::new();
    let (mut l, mut r) = (0, 0);
    while l < left.len() && r < right.len() {
        let from = left[l].from.max(right[r].from);
        let to = left[l].to.min(right[r].to);
        if from <= to {
            result.push(DateInterval { from, to });
        }
        if left[l].to < right[r].to {
            l += 1;
        } else {
            r += 1;
        }
    }
    result
}

/// Replaces the date windows among a filter's values with their sorted union.
fn merge_windows(values: &mut Vec<Value>) {
    let windows: Vec<DateInterval> = values
        .iter()
        .filter_map(|value| match value {
            Value::Window(window) => Some(*window),
            _ => None,
        })
        .collect();
    if windows.is_empty() {
        return;
    }
    values.retain(|value| !matches!(value, Value::Window(_)));
    values.extend(merge(windows).into_iter().map(Value::Window));
}

/// The value of `field` in `record`, in the form a [`Filter`] compares: text lowercased with
/// [`index_text`], `Nil` for an empty optional field.
fn record_value(field: Field, record: &CrimeRecord) -> Value {
    let text = |text: &str| Value::Text(index_text(text));
    let optional_text = |value: Option<&str>| value.map_or(Value::Nil, text);
    let optional_integer = |value: Option<i64>| value.map_or(Value::Nil, Value::Integer);
    let optional_decimal = |value: Option<f64>| value.map_or(Value::Nil, Value::Decimal);
    match field {
        Field::Id => Value::Integer(record.id),
        Field::CaseNumber => text(&record.case_number),
        Field::Date => Value::Instant(record.date),
        Field::Block => text(&record.block),
        Field::Iucr => text(&record.iucr),
        Field::PrimaryType => text(&record.primary_type),
        Field::Description => text(&record.description),
        Field::LocationDescription => optional_text(record.location_description.as_deref()),
        Field::Arrest => Value::Boolean(record.arrest),
        Field::Domestic => Value::Boolean(record.domestic),
        Field::Beat => text(&record.beat),
        Field::District => optional_text(record.district.as_deref()),
        Field::Ward => optional_integer(record.ward),
        Field::CommunityArea => optional_integer(record.community_area),
        Field::FbiCode => text(&record.fbi_code),
        Field::XCoordinate => optional_integer(record.x_coordinate),
        Field::YCoordinate => optional_integer(record.y_coordinate),
        Field::Year => Value::Integer(record.year),
        Field::UpdatedOn => Value::Instant(record.updated_on),
        Field::Latitude => optional_decimal(record.latitude),
        Field::Longitude => optional_decimal(record.longitude),
        Field::Location => optional_text(record.location().as_deref()),
    }
}
