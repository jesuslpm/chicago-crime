//! The typed query built from the raw `--where` and `--range` conditions: every validation
//! message, value typing and "sin valor", alternatives, date resolution (decision C2), the
//! non-indexed filters and the interpreted view for the JSON output (decision C9).

use std::num::NonZeroU64;

use crime_dataset::{CivilDateTime, CrimeRecord, IndexKind};
use crime_query::{Field, IndexValue, Query, RawCondition};
use serde_json::json;

const AVAILABLE: &str = "ID, Case Number, Date, IUCR, Primary Type, Location Description, Arrest, \
                         Domestic, Beat, District, Ward, Community Area, FBI Code, X Coordinate, \
                         Y Coordinate, Year, Updated On, Latitude, Longitude.";

const NEED_INDEXED: &str = "Indica al menos una condición sobre un campo indexado: ID, número de \
                            caso, fecha, tipo principal, distrito o área comunitaria.";

fn eq(field: &str, value: &str) -> RawCondition {
    RawCondition::equal(field, value)
}

fn range(from: &str, to: &str) -> RawCondition {
    RawCondition::range("Date", from, to)
}

/// An indexed condition that makes a query with non-indexed filters acceptable.
fn by_id() -> RawCondition {
    eq("ID", "1")
}

fn build(conditions: &[RawCondition]) -> Query {
    Query::build(conditions)
        .unwrap_or_else(|error| panic!("{conditions:?} should be accepted: {error}"))
}

fn error(conditions: &[RawCondition]) -> String {
    match Query::build(conditions) {
        Ok(query) => panic!("{conditions:?} should be rejected: {query:?}"),
        Err(error) => error.to_string(),
    }
}

/// The date intervals of a query as `(from, to)` ISO texts.
fn intervals(query: &Query) -> Vec<(String, String)> {
    query
        .dates()
        .expect("a date condition")
        .iter()
        .map(|interval| (interval.from.to_iso_string(), interval.to.to_iso_string()))
        .collect()
}

fn span(from: &str, to: &str) -> (String, String) {
    (from.to_owned(), to.to_owned())
}

fn values(query: &Query, field: Field) -> Vec<IndexValue> {
    query
        .equalities()
        .iter()
        .find(|equality| equality.field() == field)
        .unwrap_or_else(|| panic!("an equality on {field:?}"))
        .values()
        .to_vec()
}

fn datetime(text: &str) -> CivilDateTime {
    text.parse()
        .unwrap_or_else(|error| panic!("{text:?} should parse: {error:?}"))
}

fn record() -> CrimeRecord {
    CrimeRecord {
        id: 14348063,
        case_number: "JK438266".to_owned(),
        date: datetime("10/03/2026 12:00:00 AM"),
        block: "064XX S ELLIS AVE".to_owned(),
        iucr: "0314".to_owned(),
        primary_type: "ARSON".to_owned(),
        description: "ATTEMPT ARSON".to_owned(),
        location_description: Some("Vehicle Non-Commercial".to_owned()),
        arrest: false,
        domestic: true,
        beat: "0314".to_owned(),
        district: Some("003".to_owned()),
        ward: Some(20),
        community_area: Some(42),
        fbi_code: "08B".to_owned(),
        x_coordinate: Some(1184164),
        y_coordinate: Some(1862455),
        year: 2026,
        updated_on: datetime("10/10/2026 03:42:19 PM"),
        latitude: Some(41.777750747),
        longitude: Some(-87.600396201),
    }
}

/// The same record with every optional field empty.
fn empty_record() -> CrimeRecord {
    CrimeRecord {
        location_description: None,
        district: None,
        ward: None,
        community_area: None,
        x_coordinate: None,
        y_coordinate: None,
        latitude: None,
        longitude: None,
        ..record()
    }
}

/// Whether the filter `field = value`, next to an ID condition, accepts `record`.
fn filter_matches(field: &str, value: &str, record: &CrimeRecord) -> bool {
    build(&[by_id(), eq(field, value)]).matches(record)
}

// Validation messages.

#[test]
fn a_query_needs_an_indexed_condition() {
    assert_eq!(error(&[]), NEED_INDEXED);
    assert_eq!(error(&[eq("Arrest", "true")]), NEED_INDEXED);
    assert_eq!(
        error(&[
            eq("IUCR", "0314"),
            eq("Ward", "20"),
            eq("Updated On", "2026-10-10")
        ]),
        NEED_INDEXED
    );
}

#[test]
fn every_indexed_field_alone_is_enough() {
    for condition in [
        eq("ID", "1"),
        eq("Case Number", "JA100"),
        eq("Date", "2020-01-01"),
        range("2020-01-01", ""),
        eq("Primary Type", "THEFT"),
        eq("District", "008"),
        eq("Community Area", "42"),
    ] {
        build(&[condition, eq("Arrest", "true")]);
    }
}

#[test]
fn an_unknown_field_lists_the_conditionable_fields_with_the_name_as_typed() {
    assert_eq!(
        error(&[by_id(), eq("Fecha", "2020-01-01")]),
        format!("El campo \"Fecha\" no existe. Campos disponibles: {AVAILABLE}")
    );
    assert_eq!(
        error(&[by_id(), RawCondition::range("Desde", "2020-01-01", "")]),
        format!("El campo \"Desde\" no existe. Campos disponibles: {AVAILABLE}")
    );
}

#[test]
fn block_description_and_location_admit_no_condition() {
    for (typed, name) in [
        ("Block", "Block"),
        ("description", "Description"),
        (" LOCATION ", "Location"),
    ] {
        assert_eq!(
            error(&[by_id(), eq(typed, "x")]),
            format!("El campo \"{name}\" no admite condiciones. Campos disponibles: {AVAILABLE}")
        );
    }
}

#[test]
fn sin_valor_is_rejected_on_every_mandatory_field() {
    for name in [
        "ID",
        "Case Number",
        "Date",
        "IUCR",
        "Primary Type",
        "Arrest",
        "Domestic",
        "Beat",
        "FBI Code",
        "Year",
        "Updated On",
    ] {
        assert_eq!(
            error(&[by_id(), eq(&name.to_lowercase(), "")]),
            format!("El campo \"{name}\" es obligatorio; no admite \"sin valor\"."),
        );
    }
}

#[test]
fn sin_valor_is_accepted_on_every_optional_field() {
    for name in [
        "Location Description",
        "District",
        "Ward",
        "Community Area",
        "X Coordinate",
        "Y Coordinate",
        "Latitude",
        "Longitude",
    ] {
        let query = build(&[by_id(), eq(name, "")]);
        let applied = serde_json::to_value(query.interpreted()).expect("serializable");
        assert_eq!(applied["conditions"][1]["field"], name);
        assert_eq!(applied["conditions"][1]["values"], json!([null]), "{name}");
        let field = Field::find(name).expect("a field");
        if field.is_indexed() {
            // Resolved through the index, which keeps "sin valor" as its own value.
            assert_eq!(values(&query, field), [IndexValue::nil()], "{name}");
            assert!(query.filters().is_empty(), "{name}");
        } else {
            assert!(query.matches(&empty_record()), "{name}");
            assert!(!query.matches(&record()), "{name}");
        }
    }
}

#[test]
fn integer_fields_reject_other_values() {
    for (typed, name, value) in [
        ("ID", "ID", "abc"),
        ("community area", "Community Area", "x"),
        ("Ward", "Ward", "1.5"),
        ("X Coordinate", "X Coordinate", "1e3"),
        ("Y Coordinate", "Y Coordinate", " 7"),
        ("Year", "Year", "2020a"),
    ] {
        assert_eq!(
            error(&[by_id(), eq(typed, value)]),
            format!("\"{value}\" no es válido para \"{name}\": se esperaba un número entero.")
        );
    }
}

#[test]
fn boolean_fields_reject_other_values() {
    for (name, value) in [
        ("Arrest", "yes"),
        ("Domestic", "1"),
        ("Arrest", "verdadero"),
    ] {
        assert_eq!(
            error(&[by_id(), eq(name, value)]),
            format!("\"{value}\" no es válido para \"{name}\": se esperaba true, false, sí o no.")
        );
    }
}

#[test]
fn decimal_fields_reject_other_values() {
    for (name, value) in [
        ("Latitude", "abc"),
        ("Longitude", "41,7"),
        ("Latitude", "NaN"),
        ("Longitude", "inf"),
    ] {
        assert_eq!(
            error(&[by_id(), eq(name, value)]),
            format!("\"{value}\" no es válido para \"{name}\": se esperaba un número decimal.")
        );
    }
}

#[test]
fn ranges_are_only_admitted_on_date() {
    for name in ["Updated On", "Year", "ID", "Block"] {
        assert_eq!(
            error(&[by_id(), RawCondition::range(name, "1", "2")]),
            "Solo se admiten rangos sobre \"Date\"."
        );
    }
    build(&[RawCondition::range(" date ", "2020-01-01", "")]);
}

#[test]
fn dates_must_have_the_query_format() {
    for value in [
        "2020-1-01",
        "2020-01-1",
        "20-01-01",
        "2020/01/01",
        "01/01/2020",
        "2020-01-01 1:00",
        "2020-01-01T10:00",
        "2020-01-01  10:00",
        "2020-01-01 10:00:00",
        "2020-01-01 ",
        " 2020-01-01",
        "2020-02-30",
        "2019-02-29",
        "2020-13-01",
        "2020-01-01 24:00",
        "2020-01-01 10:60",
        "0000-01-01",
        "2020-01-01 +1:00",
        "hoy",
    ] {
        let expected =
            format!("\"{value}\" no es una fecha válida. Formato esperado: AAAA-MM-DD [hh:mm].");
        assert_eq!(error(&[eq("Date", value)]), expected, "where Date");
        assert_eq!(error(&[range(value, "")]), expected, "range from");
        assert_eq!(error(&[range("", value)]), expected, "range to");
        assert_eq!(
            error(&[by_id(), eq("Updated On", value)]),
            expected,
            "Updated On"
        );
    }
}

#[test]
fn an_inverted_range_is_rejected() {
    for (from, to) in [
        ("2020-01-02", "2020-01-01"),
        ("2020-01-01 10:00", "2020-01-01 09:59"),
        ("2020-01-02 00:00", "2020-01-01 23:59"),
    ] {
        assert_eq!(
            error(&[range(from, to)]),
            "La fecha inicial es posterior a la final.",
            "{from}..{to}"
        );
    }
}

#[test]
fn the_first_wrong_condition_in_command_line_order_is_reported() {
    assert_eq!(
        error(&[eq("Arrest", "x"), eq("Foo", "1"), by_id()]),
        "\"x\" no es válido para \"Arrest\": se esperaba true, false, sí o no."
    );
    assert_eq!(
        error(&[eq("Foo", "1"), eq("Arrest", "x"), by_id()]),
        format!("El campo \"Foo\" no existe. Campos disponibles: {AVAILABLE}")
    );
    // A wrong condition wins over the missing indexed condition.
    assert_eq!(
        error(&[eq("Ward", "x")]),
        "\"x\" no es válido para \"Ward\": se esperaba un número entero."
    );
}

// Indexed conditions.

#[test]
fn ids_are_sorted_and_deduplicated() {
    let query = build(&[
        eq("ID", "30"),
        eq("ID", "10"),
        eq("id", "30"),
        eq("ID", "5"),
    ]);
    assert_eq!(query.ids(), Some(&[5, 10, 30][..]));
    assert!(query.equalities().is_empty());
    assert_eq!(query.dates(), None);
    assert!(build(&[eq("Primary Type", "x")]).ids().is_none());
}

#[test]
fn field_names_match_case_insensitively_and_values_are_taken_as_given() {
    let query = build(&[eq(" primary TYPE  ", " THEFT")]);
    assert_eq!(
        values(&query, Field::PrimaryType),
        [IndexValue::text(" theft")]
    );
    assert_eq!(query.equalities()[0].index(), IndexKind::PrimaryType);
}

#[test]
fn alternatives_on_one_field_are_deduplicated_after_lowercasing() {
    let query = build(&[
        eq("Primary Type", "theft"),
        eq("Primary Type", "THEFT"),
        eq("Primary Type", "Battery"),
        eq("primary type", "Theft"),
    ]);
    assert_eq!(query.equalities().len(), 1);
    assert_eq!(
        values(&query, Field::PrimaryType),
        [IndexValue::text("theft"), IndexValue::text("battery")]
    );
}

#[test]
fn different_fields_are_separate_conditions_in_first_appearance_order() {
    let query = build(&[
        eq("District", "008"),
        eq("Case Number", "JA100"),
        eq("District", "8"),
        eq("Community Area", "42"),
        eq("Community Area", ""),
        eq("District", ""),
    ]);
    let fields: Vec<Field> = query.equalities().iter().map(|e| e.field()).collect();
    assert_eq!(
        fields,
        [Field::District, Field::CaseNumber, Field::CommunityArea]
    );
    // Codes keep their leading zeros: `8` is not `008`.
    assert_eq!(
        values(&query, Field::District),
        [
            IndexValue::text("008"),
            IndexValue::text("8"),
            IndexValue::nil()
        ]
    );
    assert_eq!(
        values(&query, Field::CaseNumber),
        [IndexValue::text("ja100")]
    );
    assert_eq!(
        values(&query, Field::CommunityArea),
        [IndexValue::integer(42), IndexValue::nil()]
    );
    let indexes: Vec<IndexKind> = query.equalities().iter().map(|e| e.index()).collect();
    assert_eq!(
        indexes,
        [
            IndexKind::District,
            IndexKind::CaseNumber,
            IndexKind::CommunityArea
        ]
    );
}

// Dates (decision C2).

#[test]
fn a_from_without_time_starts_at_midnight_and_a_to_without_time_includes_the_whole_day() {
    assert_eq!(
        intervals(&build(&[range("2020-01-01", "2020-01-31")])),
        [span("2020-01-01T00:00:00", "2020-01-31T23:59:59")]
    );
}

#[test]
fn a_to_with_time_includes_the_whole_minute_and_a_from_with_time_starts_on_it() {
    assert_eq!(
        intervals(&build(&[range("2020-01-01 10:30", "2020-01-31 18:05")])),
        [span("2020-01-01T10:30:00", "2020-01-31T18:05:59")]
    );
    assert_eq!(
        intervals(&build(&[range("2020-01-01 10:30", "2020-01-01 10:30")])),
        [span("2020-01-01T10:30:00", "2020-01-01T10:30:59")]
    );
    assert_eq!(
        intervals(&build(&[range("2020-01-01 10:30", "2020-01-01")])),
        [span("2020-01-01T10:30:00", "2020-01-01T23:59:59")]
    );
}

#[test]
fn a_range_may_give_only_one_end() {
    assert_eq!(
        intervals(&build(&[range("2020-01-01", "")])),
        [span("2020-01-01T00:00:00", "9999-12-31T23:59:59")]
    );
    assert_eq!(
        intervals(&build(&[range("", "2020-01-31 00:00")])),
        [span("0001-01-01T00:00:00", "2020-01-31T00:00:59")]
    );
}

#[test]
fn date_equality_covers_the_whole_day_or_the_whole_minute() {
    assert_eq!(
        intervals(&build(&[eq("Date", "2020-01-02")])),
        [span("2020-01-02T00:00:00", "2020-01-02T23:59:59")]
    );
    assert_eq!(
        intervals(&build(&[eq("date", "2020-01-02 23:59")])),
        [span("2020-01-02T23:59:00", "2020-01-02T23:59:59")]
    );
}

#[test]
fn date_equality_alternatives_merge_into_sorted_intervals() {
    // A minute inside a day adds nothing; contiguous days join; separate days stay apart.
    let query = build(&[
        eq("Date", "2020-01-05"),
        eq("Date", "2020-01-02 10:30"),
        eq("Date", "2020-01-02"),
        eq("Date", "2020-01-03"),
        eq("Date", "2020-01-05 00:00"),
    ]);
    assert_eq!(
        intervals(&query),
        [
            span("2020-01-02T00:00:00", "2020-01-03T23:59:59"),
            span("2020-01-05T00:00:00", "2020-01-05T23:59:59"),
        ]
    );
}

#[test]
fn several_ranges_intersect() {
    let query = build(&[range("2020-01-01", "2020-01-31"), range("2020-01-15", "")]);
    assert_eq!(
        intervals(&query),
        [span("2020-01-15T00:00:00", "2020-01-31T23:59:59")]
    );
}

#[test]
fn equality_alternatives_and_ranges_intersect() {
    let query = build(&[
        eq("Date", "2020-01-02"),
        range("2020-01-02 12:00", "2020-01-05 06:00"),
        eq("Date", "2020-01-05"),
        eq("Date", "2020-01-09"),
    ]);
    assert_eq!(
        intervals(&query),
        [
            span("2020-01-02T12:00:00", "2020-01-02T23:59:59"),
            span("2020-01-05T00:00:00", "2020-01-05T06:00:59"),
        ]
    );
}

#[test]
fn disjoint_date_conditions_are_a_valid_query_with_no_interval() {
    let query = build(&[range("2020-01-01", "2020-01-31"), range("2020-03-01", "")]);
    assert_eq!(query.dates(), Some(&[][..]));
}

#[test]
fn a_repeated_date_is_one_interval() {
    let query = build(&[eq("Date", "2020-01-01"), eq("Date", "2020-01-01")]);
    assert_eq!(
        intervals(&query),
        [span("2020-01-01T00:00:00", "2020-01-01T23:59:59")]
    );
}

// Non-indexed filters.

#[test]
fn text_filters_lowercase_both_sides_and_never_pad_codes() {
    let record = record();
    assert!(filter_matches(
        "Location Description",
        "vehicle NON-commercial",
        &record
    ));
    assert!(!filter_matches("Location Description", "vehicle", &record));
    assert!(filter_matches("IUCR", "0314", &record));
    assert!(!filter_matches("IUCR", "314", &record));
    assert!(filter_matches("Beat", "0314", &record));
    assert!(filter_matches("FBI Code", "08b", &record));
    assert!(!filter_matches("FBI Code", "8B", &record));
    assert!(!filter_matches("Location Description", "", &record));
    assert!(filter_matches("Location Description", "", &empty_record()));
}

#[test]
fn boolean_filters_accept_every_spelling() {
    let record = record(); // arrest: false, domestic: true
    for value in ["true", "TRUE", "True", "sí", "SÍ", "Sí", "si", "SI"] {
        assert!(filter_matches("Domestic", value, &record), "{value}");
        assert!(!filter_matches("Arrest", value, &record), "{value}");
    }
    for value in ["false", "FALSE", "no", "No", "NO"] {
        assert!(filter_matches("Arrest", value, &record), "{value}");
        assert!(!filter_matches("Domestic", value, &record), "{value}");
    }
}

#[test]
fn integer_and_decimal_filters_compare_numbers() {
    let record = record();
    assert!(filter_matches("Ward", "20", &record));
    assert!(filter_matches("Ward", "020", &record));
    assert!(!filter_matches("Ward", "21", &record));
    assert!(!filter_matches("Ward", "", &record));
    assert!(filter_matches("Ward", "", &empty_record()));
    assert!(filter_matches("X Coordinate", "1184164", &record));
    assert!(filter_matches("Y Coordinate", "1862455", &record));
    assert!(filter_matches("Year", "2026", &record));
    assert!(!filter_matches("Year", "2025", &record));
    assert!(filter_matches("Latitude", "41.777750747", &record));
    assert!(filter_matches("Longitude", "-87.600396201", &record));
    assert!(!filter_matches("Latitude", "41.77775", &record));
    assert!(!filter_matches("Latitude", "", &record));
    assert!(filter_matches("Longitude", "", &empty_record()));
}

#[test]
fn updated_on_equality_covers_the_day_or_the_minute() {
    let record = record(); // updated 10/10/2026 03:42:19 PM
    assert!(filter_matches("Updated On", "2026-10-10", &record));
    assert!(filter_matches("Updated On", "2026-10-10 15:42", &record));
    assert!(!filter_matches("Updated On", "2026-10-10 15:43", &record));
    assert!(!filter_matches("Updated On", "2026-10-11", &record));
}

#[test]
fn alternatives_of_a_filter_are_or_and_filters_are_and() {
    let record = record();
    let query = build(&[
        by_id(),
        eq("IUCR", "0486"),
        eq("Arrest", "no"),
        eq("IUCR", "0314"),
    ]);
    assert!(query.matches(&record));
    let names: Vec<&str> = query.filters().iter().map(|f| f.field().name()).collect();
    assert_eq!(names, ["IUCR", "Arrest"]);
    assert!(query.filters()[0].matches(&record));

    let query = build(&[by_id(), eq("IUCR", "0314"), eq("Arrest", "true")]);
    assert!(!query.matches(&record));
    assert!(query.filters()[0].matches(&record));
    assert!(!query.filters()[1].matches(&record));
}

#[test]
fn indexed_conditions_are_not_filters() {
    let query = build(&[
        eq("ID", "1"),
        eq("Case Number", "x"),
        eq("Date", "2020-01-01"),
        eq("Primary Type", "x"),
        eq("District", "x"),
        eq("Community Area", "1"),
    ]);
    assert!(query.filters().is_empty());
    // With no filter every record passes: the indexes already decided.
    assert!(query.matches(&record()));
}

// Limit and the interpreted query (decision C9).

#[test]
fn the_limit_is_kept_with_the_query() {
    let query = build(&[by_id()]);
    assert_eq!(query.limit(), None);
    let limit = NonZeroU64::new(5);
    assert_eq!(query.with_limit(limit).limit(), limit);
}

#[test]
fn the_interpreted_query_shows_every_condition_as_applied() {
    let query = build(&[
        eq("Primary Type", "THEFT"),
        eq("ID", "30"),
        range("2020-01-01", "2020-01-31 10:00"),
        eq("District", ""),
        eq("Arrest", "Sí"),
        eq("Location Description", "Street"),
        eq("ID", "10"),
        eq("District", "008"),
        eq("Latitude", "41.5"),
        eq("Ward", "20"),
        eq("Updated On", "2026-10-10 15:42"),
        eq("Community Area", "42"),
    ])
    .with_limit(NonZeroU64::new(3));

    let applied = serde_json::to_value(query.interpreted()).expect("serializable");

    assert_eq!(
        applied,
        json!({
            "conditions": [
                {"field": "Primary Type", "indexed": true, "values": ["theft"]},
                {"field": "ID", "indexed": true, "values": [10, 30]},
                {"field": "Date", "indexed": true, "intervals": [
                    {"from": "2020-01-01T00:00:00", "to": "2020-01-31T10:00:59"}
                ]},
                {"field": "District", "indexed": true, "values": [null, "008"]},
                {"field": "Arrest", "indexed": false, "values": [true]},
                {"field": "Location Description", "indexed": false, "values": ["street"]},
                {"field": "Latitude", "indexed": false, "values": [41.5]},
                {"field": "Ward", "indexed": false, "values": [20]},
                {"field": "Updated On", "indexed": false, "intervals": [
                    {"from": "2026-10-10T15:42:00", "to": "2026-10-10T15:42:59"}
                ]},
                {"field": "Community Area", "indexed": true, "values": [42]},
            ],
            "limit": 3,
        })
    );
    let unlimited = serde_json::to_value(build(&[by_id()]).interpreted()).expect("serializable");
    assert_eq!(unlimited["limit"], json!(null));
}

#[test]
fn an_empty_date_condition_is_shown_with_no_interval() {
    let query = build(&[range("2020-01-01", "2020-01-31"), range("2020-03-01", "")]);
    let applied = serde_json::to_value(query.interpreted()).expect("serializable");
    assert_eq!(
        applied["conditions"],
        json!([{"field": "Date", "indexed": true, "intervals": []}])
    );
}
