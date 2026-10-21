//! Query execution through the library (decision C10): every indexed condition resolved through
//! its index, alternatives joined, conditions intersected, filters applied to the records found,
//! the limit, the warnings for missing IDs and case numbers, read errors and cancellation.
//!
//! Every result is compared with the set the test computes from the fixture rows themselves.

mod common;

use std::cell::Cell;
use std::collections::BTreeSet;
use std::fs::OpenOptions;
use std::num::NonZeroU64;
use std::path::Path;
use std::time::Duration;

use common::{Fixture, Row, datetime, fixture_rows, many_rows, row};
use crime_dataset::{CivilDateTime, HEADER};
use crime_query::{Dataset, ExecError, Execution, Field, Query, RawCondition, messages};

fn eq(field: &str, value: &str) -> RawCondition {
    RawCondition::equal(field, value)
}

fn range(from: &str, to: &str) -> RawCondition {
    RawCondition::range("Date", from, to)
}

fn never() -> bool {
    false
}

fn open(fixture: &Fixture) -> Dataset {
    Dataset::open(&fixture.output()).expect("the fixture dataset opens")
}

fn query(conditions: &[RawCondition], limit: Option<u64>) -> Query {
    Query::build(conditions)
        .expect("the test query is valid")
        .with_limit(limit.and_then(NonZeroU64::new))
}

/// The IDs of the records a query returns, and its warnings.
fn results(dataset: &Dataset, query: &Query) -> (BTreeSet<i64>, Vec<String>, u64) {
    let mut execution = Execution::start(dataset, query, &never).expect("the execution starts");
    let warnings = execution.warnings().to_vec();
    let mut ids = BTreeSet::new();
    let mut returned = 0;
    for record in &mut execution {
        let record = record.expect("no read fails");
        assert!(ids.insert(record.id), "{} returned twice", record.id);
        returned += 1;
    }
    assert_eq!(execution.summary().count, returned);
    (ids, warnings, returned)
}

fn ids_of(dataset: &Dataset, conditions: &[RawCondition]) -> BTreeSet<i64> {
    results(dataset, &query(conditions, None)).0
}

fn count_of(dataset: &Dataset, query: &Query) -> u64 {
    Execution::start(dataset, query, &never)
        .expect("the execution starts")
        .count()
        .expect("no read fails")
        .count
}

/// The value of a column of a fixture row, by CSV name.
fn col<'r>(row: &'r Row, name: &str) -> &'r str {
    let column = HEADER
        .iter()
        .position(|&column| column == name)
        .unwrap_or_else(|| panic!("{name} is a column"));
    &row[column]
}

fn id(row: &Row) -> i64 {
    col(row, "ID").parse().expect("an integer ID")
}

/// The IDs of the rows that meet `predicate`, computed without the query app.
fn expected(rows: &[Row], predicate: impl Fn(&Row) -> bool) -> BTreeSet<i64> {
    rows.iter().filter(|row| predicate(row)).map(id).collect()
}

fn lower(row: &Row, name: &str) -> String {
    col(row, name).to_lowercase()
}

fn date(row: &Row) -> CivilDateTime {
    datetime(col(row, "Date"))
}

fn at(text: &str) -> CivilDateTime {
    datetime(text)
}

#[test]
fn each_indexed_condition_alone_finds_its_records() {
    let rows = fixture_rows();
    let fixture = Fixture::with_rows(&rows);
    let dataset = open(&fixture);

    assert_eq!(
        ids_of(&dataset, &[eq("ID", "15")]),
        expected(&rows, |r| id(r) == 15)
    );
    assert_eq!(
        ids_of(&dataset, &[eq("Case Number", "ja100")]),
        expected(&rows, |r| lower(r, "Case Number") == "ja100")
    );
    assert_eq!(
        ids_of(&dataset, &[eq("Date", "2020-01-01")]),
        expected(&rows, |r| {
            at("01/01/2020 12:00:00 AM") <= date(r) && date(r) <= at("01/01/2020 11:59:59 PM")
        })
    );
    assert_eq!(
        ids_of(&dataset, &[eq("Primary Type", "THEFT")]),
        expected(&rows, |r| lower(r, "Primary Type") == "theft")
    );
    assert_eq!(
        ids_of(&dataset, &[eq("District", "008")]),
        expected(&rows, |r| col(r, "District") == "008")
    );
    assert_eq!(
        ids_of(&dataset, &[eq("Community Area", "42")]),
        expected(&rows, |r| col(r, "Community Area") == "42")
    );
    // Sanity: the expected sets are not trivial.
    assert_eq!(
        ids_of(&dataset, &[eq("Case Number", "JA100")]),
        [10, 20].into()
    );
    assert_eq!(
        ids_of(&dataset, &[eq("District", "008")]),
        [10, 15, 40].into()
    );
}

#[test]
fn alternatives_are_joined_without_duplicates() {
    let rows = fixture_rows();
    let fixture = Fixture::with_rows(&rows);
    let dataset = open(&fixture);

    let got = ids_of(
        &dataset,
        &[
            eq("Primary Type", "THEFT"),
            eq("Primary Type", "battery"),
            eq("primary type", "Theft"),
        ],
    );
    assert_eq!(
        got,
        expected(&rows, |r| ["theft", "battery"]
            .contains(&lower(r, "Primary Type").as_str()))
    );
    assert_eq!(got, [10, 20, 30, 40, 50].into());

    // Codes keep their leading zeros: `8` and `008` are two values.
    assert_eq!(
        ids_of(&dataset, &[eq("District", "008"), eq("District", "8")]),
        expected(&rows, |r| ["008", "8"].contains(&col(r, "District")))
    );
    assert_eq!(
        ids_of(
            &dataset,
            &[
                eq("Case Number", "JA100"),
                eq("Case Number", "ja500"),
                eq("Case Number", "ja100")
            ]
        ),
        [10, 20, 50].into()
    );
}

#[test]
fn several_indexed_conditions_without_a_date_are_intersected() {
    let rows = fixture_rows();
    let fixture = Fixture::with_rows(&rows);
    let dataset = open(&fixture);

    assert_eq!(
        ids_of(
            &dataset,
            &[eq("Primary Type", "theft"), eq("District", "008")]
        ),
        expected(&rows, |r| lower(r, "Primary Type") == "theft"
            && col(r, "District") == "008")
    );
    let three = ids_of(
        &dataset,
        &[
            eq("Primary Type", "theft"),
            eq("Community Area", "42"),
            eq("Community Area", "7"),
            eq("Case Number", "JA300"),
            eq("Case Number", "JA100"),
        ],
    );
    assert_eq!(
        three,
        expected(&rows, |r| {
            lower(r, "Primary Type") == "theft"
                && ["42", "7"].contains(&col(r, "Community Area"))
                && ["ja300", "ja100"].contains(&lower(r, "Case Number").as_str())
        })
    );
    assert_eq!(three, [10, 30].into());
    assert_eq!(
        ids_of(
            &dataset,
            &[
                eq("ID", "10"),
                eq("ID", "15"),
                eq("District", "008"),
                eq("Primary Type", "theft")
            ]
        ),
        [10].into()
    );
}

#[test]
fn a_date_condition_is_combined_with_the_other_indexed_conditions() {
    let rows = fixture_rows();
    let fixture = Fixture::with_rows(&rows);
    let dataset = open(&fixture);
    let in_jan_1_to_2 = |r: &Row| {
        at("01/01/2020 12:00:00 AM") <= date(r) && date(r) <= at("01/02/2020 11:59:59 PM")
    };

    assert_eq!(
        ids_of(
            &dataset,
            &[
                range("2020-01-01", "2020-01-02"),
                eq("Primary Type", "theft")
            ]
        ),
        expected(&rows, |r| in_jan_1_to_2(r)
            && lower(r, "Primary Type") == "theft")
    );
    let three = ids_of(
        &dataset,
        &[
            range("2020-01-01", "2020-01-02"),
            eq("Primary Type", "theft"),
            eq("District", "008"),
            eq("District", ""),
        ],
    );
    assert_eq!(
        three,
        expected(&rows, |r| {
            in_jan_1_to_2(r)
                && lower(r, "Primary Type") == "theft"
                && ["008", ""].contains(&col(r, "District"))
        })
    );
    assert_eq!(three, [10, 30].into());
    assert_eq!(
        ids_of(
            &dataset,
            &[eq("ID", "20"), eq("ID", "40"), range("2020-01-01", "")]
        ),
        [20, 40].into()
    );
    assert_eq!(
        ids_of(
            &dataset,
            &[eq("Case Number", "JA100"), range("", "2020-01-01 00:00")]
        ),
        // A "hasta" with a time includes its whole minute.
        [10, 20].into()
    );
}

#[test]
fn a_date_condition_of_several_intervals_reads_each_one() {
    let rows = fixture_rows();
    let fixture = Fixture::with_rows(&rows);
    let dataset = open(&fixture);

    let got = ids_of(
        &dataset,
        &[
            eq("Date", "2019-12-31"),
            eq("Date", "2020-01-03"),
            eq("Primary Type", "battery"),
            eq("Primary Type", "arson"),
        ],
    );
    assert_eq!(
        got,
        expected(&rows, |r| {
            let day = col(r, "Date").get(..10).unwrap_or_default().to_owned();
            ["12/31/2019", "01/03/2020"].contains(&day.as_str())
                && ["battery", "arson"].contains(&lower(r, "Primary Type").as_str())
        })
    );
    assert_eq!(got, [5, 40, 50].into());
    // Ranges that leave no instant: zero results.
    assert!(
        ids_of(
            &dataset,
            &[eq("Date", "2020-01-01"), range("2020-01-02", "")]
        )
        .is_empty()
    );
}

#[test]
fn missing_ids_are_warned_and_the_existing_ones_returned() {
    let fixture = Fixture::with_rows(&fixture_rows());
    let dataset = open(&fixture);

    let (ids, warnings, _) = results(
        &dataset,
        &query(
            &[
                eq("ID", "999"),
                eq("ID", "10"),
                eq("ID", "7"),
                eq("ID", "20"),
            ],
            None,
        ),
    );
    assert_eq!(ids, [10, 20].into());
    assert_eq!(
        warnings,
        [
            "No existe ningún delito con ID 7.",
            "No existe ningún delito con ID 999."
        ]
    );

    // The warning does not depend on the other conditions; a single missing ID is no error.
    let (ids, warnings, _) = results(
        &dataset,
        &query(&[eq("ID", "123"), range("2020-01-01", "")], None),
    );
    assert!(ids.is_empty());
    assert_eq!(warnings, ["No existe ningún delito con ID 123."]);
}

#[test]
fn a_missing_case_number_is_warned_as_typed() {
    let fixture = Fixture::with_rows(&fixture_rows());
    let dataset = open(&fixture);

    let (ids, warnings, _) = results(
        &dataset,
        &query(
            &[eq("Case Number", "Ja000000"), eq("Case Number", "ja100")],
            None,
        ),
    );
    assert_eq!(ids, [10, 20].into());
    assert_eq!(
        warnings,
        ["No existe ningún delito con número de caso Ja000000."]
    );

    let (ids, warnings, _) = results(&dataset, &query(&[eq("Case Number", "JA000000")], None));
    assert!(ids.is_empty());
    assert_eq!(
        warnings,
        ["No existe ningún delito con número de caso JA000000."]
    );
}

#[test]
fn sin_valor_finds_the_records_without_district_or_community_area() {
    let rows = fixture_rows();
    let fixture = Fixture::with_rows(&rows);
    let dataset = open(&fixture);

    assert_eq!(
        ids_of(&dataset, &[eq("District", "")]),
        expected(&rows, |r| col(r, "District").is_empty())
    );
    assert_eq!(
        ids_of(&dataset, &[eq("Community Area", "")]),
        expected(&rows, |r| col(r, "Community Area").is_empty())
    );
    assert_eq!(
        ids_of(&dataset, &[eq("Community Area", "")]),
        [20, 40].into()
    );
}

/// Rows with varied arrests and location descriptions in mixed case.
fn filter_rows() -> Vec<Row> {
    let specs = [
        ("1", "THEFT", "true", "STREET", "008"),
        ("2", "theft", "false", "street", "008"),
        ("3", "Theft", "true", "Street", ""),
        ("4", "THEFT", "false", "APARTMENT", "008"),
        ("5", "BATTERY", "true", "STREET", "008"),
        ("6", "THEFT", "true", "", "001"),
        ("7", "theft", "true", "street", "008"),
    ];
    specs
        .iter()
        .map(|&(id, kind, arrest, place, district)| {
            row(&[
                ("ID", id),
                ("Case Number", &format!("JC{id}")),
                ("Primary Type", kind),
                ("Arrest", arrest),
                ("Location Description", place),
                ("District", district),
            ])
        })
        .collect()
}

#[test]
fn filters_apply_to_the_records_the_indexes_found() {
    let rows = filter_rows();
    let fixture = Fixture::with_rows(&rows);
    let dataset = open(&fixture);

    assert_eq!(
        ids_of(
            &dataset,
            &[eq("Primary Type", "theft"), eq("Arrest", "true")]
        ),
        expected(&rows, |r| lower(r, "Primary Type") == "theft"
            && col(r, "Arrest") == "true")
    );
    let street = expected(&rows, |r| {
        lower(r, "Primary Type") == "theft"
            && col(r, "District") == "008"
            && lower(r, "Location Description") == "street"
    });
    assert_eq!(street, [1, 2, 7].into());
    for spelling in ["street", "STREET", "Street"] {
        let conditions = [
            eq("Primary Type", "theft"),
            eq("District", "008"),
            eq("Location Description", spelling),
        ];
        assert_eq!(ids_of(&dataset, &conditions), street, "{spelling}");
        assert_eq!(
            count_of(&dataset, &query(&conditions, None)),
            3,
            "{spelling}"
        );
    }
    assert_eq!(
        ids_of(
            &dataset,
            &[eq("District", "001"), eq("Location Description", "")]
        ),
        [6].into()
    );
}

#[test]
fn an_empty_result_completes_with_zero() {
    let fixture = Fixture::with_rows(&fixture_rows());
    let dataset = open(&fixture);

    let query = query(&[eq("Primary Type", "homicide")], None);
    let (ids, warnings, _) = results(&dataset, &query);
    assert!(ids.is_empty());
    assert!(warnings.is_empty());
    let summary = Execution::start(&dataset, &query, &never)
        .expect("starts")
        .count()
        .expect("no read fails");
    assert_eq!(summary.count, 0);
    assert!(
        summary.stats_line().starts_with("0 resultados en "),
        "{}",
        summary.stats_line()
    );
}

#[test]
fn a_limit_returns_exactly_n_results_of_the_unlimited_result() {
    let rows = many_rows(400);
    let fixture = Fixture::with_rows(&rows);
    let dataset = open(&fixture);
    let conditions = [eq("Primary Type", "theft"), eq("Arrest", "false")];
    let all = ids_of(&dataset, &conditions);
    assert_eq!(all.len(), 400);

    for limit in [1, 7, 399] {
        let (ids, _, returned) = results(&dataset, &query(&conditions, Some(limit)));
        assert_eq!(returned, limit);
        assert!(ids.is_subset(&all));
        assert_eq!(count_of(&dataset, &query(&conditions, Some(limit))), limit);
    }
    let with_dates = [range("2020-01-02", "2020-01-05")];
    let unlimited = ids_of(&dataset, &with_dates);
    let (ids, _, returned) = results(&dataset, &query(&with_dates, Some(5)));
    assert_eq!(returned, 5);
    assert!(ids.is_subset(&unlimited));
    // A limit above the result returns the whole result.
    assert_eq!(count_of(&dataset, &query(&conditions, Some(10_000))), 400);
}

#[test]
fn the_summary_reports_the_indexes_and_filters_in_csv_order() {
    let fixture = Fixture::with_rows(&fixture_rows());
    let dataset = open(&fixture);

    let query = query(
        &[
            eq("Community Area", "42"),
            eq("Arrest", "false"),
            range("2020-01-01", ""),
            eq("Primary Type", "theft"),
            eq("Ward", "20"),
            eq("ID", "10"),
            eq("Case Number", "JA100"),
            eq("District", "008"),
        ],
        None,
    );
    let summary = Execution::start(&dataset, &query, &never)
        .expect("starts")
        .count()
        .expect("no read fails");
    assert_eq!(summary.count, 1);
    assert_eq!(
        summary.indexes,
        [
            Field::Id,
            Field::CaseNumber,
            Field::Date,
            Field::PrimaryType,
            Field::District,
            Field::CommunityArea
        ]
    );
    assert_eq!(summary.filters, [Field::Arrest, Field::Ward]);
    let line = summary.stats_line();
    assert!(line.starts_with("1 resultado en "), "{line}");
    assert!(
        line.ends_with(
            " s. Índices usados: ID, Case Number, Date, Primary Type, District, Community Area. \
             Filtros sin índice: Arrest, Ward."
        ),
        "{line}"
    );
}

#[test]
fn the_stats_line_uses_spanish_number_formats() {
    let indexes = ["Primary Type"];
    assert_eq!(
        messages::stats(1_838_502, Duration::from_millis(123), &indexes, &[]),
        "1.838.502 resultados en 0,123 s. Índices usados: Primary Type. Filtros sin índice: \
         ninguno."
    );
    assert_eq!(
        messages::stats(
            1,
            Duration::from_micros(2_345_678),
            &["ID"],
            &["Arrest", "Ward"]
        ),
        "1 resultado en 2,346 s. Índices usados: ID. Filtros sin índice: Arrest, Ward."
    );
    assert_eq!(
        messages::stats(0, Duration::ZERO, &["Date"], &[]),
        "0 resultados en 0,000 s. Índices usados: Date. Filtros sin índice: ninguno."
    );
    assert_eq!(
        messages::stats(999, Duration::from_millis(1_234_005), &["Date"], &[]),
        "999 resultados en 1.234,005 s. Índices usados: Date. Filtros sin índice: ninguno."
    );
}

/// Cuts a table file of the open dataset to half its size, as a failing disk would leave it.
fn truncate_to_half(path: &Path) {
    let file = OpenOptions::new()
        .write(true)
        .open(path)
        .expect("the table can be opened for writing");
    let size = file.metadata().expect("readable").len();
    file.set_len(size / 2).expect("the table can be truncated");
}

#[test]
fn a_read_error_ends_the_execution_without_a_count() {
    let fixture = Fixture::with_rows(&many_rows(3_000));
    let dataset = open(&fixture);
    truncate_to_half(&fixture.version_dir().join("primary.sst"));
    let unreadable = format!(
        "No se pudo leer el conjunto de datos en {}.",
        fixture.output().display()
    );

    let filtered = query(&[eq("Primary Type", "theft"), eq("Arrest", "false")], None);
    let error = Execution::start(&dataset, &filtered, &never)
        .expect("starts")
        .count()
        .expect_err("the count should fail");
    assert!(matches!(error, ExecError::Read(_)), "{error:?}");
    assert_eq!(error.to_string(), unreadable);

    let mut execution = Execution::start(&dataset, &filtered, &never).expect("starts");
    let items: Vec<_> = (&mut execution).collect();
    let (last, before) = items.split_last().expect("at least the error");
    assert!(matches!(last, Err(ExecError::Read(_))), "{last:?}");
    assert!(before.iter().all(Result::is_ok));
    assert!(before.len() < 3_000);
    assert!(execution.next().is_none(), "nothing follows an error");
}

#[test]
fn a_read_error_in_an_index_scan_ends_the_execution() {
    let fixture = Fixture::with_rows(&many_rows(3_000));
    let dataset = open(&fixture);
    truncate_to_half(&fixture.version_dir().join("index-date.sst"));

    let dated = query(&[range("2020-01-01", "")], None);
    let error = Execution::start(&dataset, &dated, &never)
        .expect("starts")
        .count()
        .expect_err("the count should fail");
    assert!(matches!(error, ExecError::Read(_)), "{error:?}");
}

#[test]
fn a_missing_id_check_that_cannot_read_fails_to_start() {
    let fixture = Fixture::with_rows(&many_rows(3_000));
    let dataset = open(&fixture);
    truncate_to_half(&fixture.version_dir().join("primary.sst"));

    let by_id = query(&[eq("ID", "2990")], None);
    assert!(Execution::start(&dataset, &by_id, &never).is_err());
}

#[test]
fn cancellation_stops_the_execution() {
    let fixture = Fixture::with_rows(&many_rows(400));
    let dataset = open(&fixture);
    let polls = Cell::new(0_u32);
    let cancel = || {
        polls.set(polls.get() + 1);
        polls.get() > 20
    };

    for conditions in [
        vec![eq("Primary Type", "theft")],
        vec![range("2020-01-01", "")],
        vec![eq("Primary Type", "theft"), eq("Arrest", "true")],
    ] {
        polls.set(0);
        let query = query(&conditions, None);
        let mut execution = Execution::start(&dataset, &query, &cancel).expect("starts");
        let items: Vec<_> = (&mut execution).collect();
        let (last, before) = items.split_last().expect("at least the cancellation");
        assert!(matches!(last, Err(ExecError::Cancelled)), "{last:?}");
        // Every result takes at least one poll.
        assert!(before.len() <= 20, "{}", before.len());
        assert!(execution.next().is_none());
        assert_eq!(
            last.as_ref().unwrap_err().to_string(),
            "Consulta cancelada."
        );

        polls.set(0);
        let error = Execution::start(&dataset, &query, &cancel)
            .expect("starts")
            .count()
            .expect_err("cancelled");
        assert!(matches!(error, ExecError::Cancelled));
    }
}

#[test]
fn cancellation_is_polled_during_the_existence_checks() {
    let fixture = Fixture::with_rows(&fixture_rows());
    let dataset = open(&fixture);
    let polls = Cell::new(0_u32);
    let raised_from = Cell::new(u32::MAX);
    let cancel = || {
        polls.set(polls.get() + 1);
        polls.get() >= raised_from.get()
    };
    // Three IDs and two case numbers: one poll before each of the five existence checks.
    let query = query(
        &[
            eq("ID", "10"),
            eq("ID", "999"),
            eq("ID", "20"),
            eq("Case Number", "JA100"),
            eq("Case Number", "JA000000"),
        ],
        None,
    );
    for raised in 1..=5 {
        polls.set(0);
        raised_from.set(raised);
        let error = Execution::start(&dataset, &query, &cancel)
            .err()
            .unwrap_or_else(|| panic!("cancelled at poll {raised}"));
        assert!(matches!(error, ExecError::Cancelled), "{error:?}");
        assert_eq!(polls.get(), raised, "no check runs after the cancellation");
    }

    polls.set(0);
    raised_from.set(6);
    let execution = Execution::start(&dataset, &query, &cancel).expect("starts");
    assert_eq!(polls.get(), 5);
    assert_eq!(execution.warnings().len(), 2, "{:?}", execution.warnings());
}

/// A small deterministic generator (xorshift64*), so the property check needs no dependency.
struct Random(u64);

impl Random {
    fn below(&mut self, n: usize) -> usize {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        (self.0.wrapping_mul(0x2545_f491_4f6c_dd1d) >> 33) as usize % n
    }

    fn pick<'a>(&mut self, values: &[&'a str]) -> &'a str {
        values[self.below(values.len())]
    }

    fn chance(&mut self, percent: usize) -> bool {
        self.below(100) < percent
    }
}

const TYPES: [&str; 6] = ["THEFT", "theft", "Battery", "BATTERY", "ARSON", "Assault"];
const DISTRICTS: [&str; 5] = ["001", "008", "8", "", "011"];
const AREAS: [&str; 5] = ["1", "7", "42", "", "77"];
const PLACES: [&str; 5] = ["STREET", "street", "APARTMENT", "", "Residence"];
const BOOLEANS: [&str; 2] = ["true", "false"];

/// `count` rows with IDs spread out, values drawn from small domains in mixed case, shared case
/// numbers and dates over ten days.
fn random_rows(random: &mut Random, count: usize) -> Vec<Row> {
    (0..count)
        .map(|n| {
            let id = (n * 7 + 3).to_string();
            let case = format!("JD{}", random.below(count / 2));
            let date = CivilDateTime::from_parts(
                2020,
                1,
                1 + random.below(10) as i64,
                random.below(24) as i64,
                random.below(60) as i64,
                random.below(60) as i64,
            )
            .expect("a valid date")
            .to_string();
            row(&[
                ("ID", &id),
                ("Case Number", &case),
                ("Date", &date),
                ("Primary Type", random.pick(&TYPES)),
                ("District", random.pick(&DISTRICTS)),
                ("Community Area", random.pick(&AREAS)),
                ("Location Description", random.pick(&PLACES)),
                ("Arrest", random.pick(&BOOLEANS)),
            ])
        })
        .collect()
}

/// One condition of a random query and the check it makes on a fixture row.
type Check = Box<dyn Fn(&Row) -> bool>;

fn alternatives(
    random: &mut Random,
    field: &'static str,
    domain: &[&str],
    conditions: &mut Vec<RawCondition>,
    checks: &mut Vec<Check>,
) {
    let wanted: Vec<String> = (0..1 + random.below(2))
        .map(|_| random.pick(domain).to_owned())
        .collect();
    for value in &wanted {
        conditions.push(eq(field, value));
    }
    let wanted: Vec<String> = wanted.iter().map(|value| value.to_lowercase()).collect();
    checks.push(Box::new(move |row| wanted.contains(&lower(row, field))));
}

#[test]
fn the_plan_returns_what_a_brute_force_filter_of_the_rows_returns() {
    let mut random = Random(0x9e37_79b9_7f4a_7c15);
    let rows = random_rows(&mut random, 3_000);
    let fixture = Fixture::with_rows(&rows);
    let dataset = open(&fixture);
    let mut non_empty = 0;

    for round in 0..60 {
        let mut conditions = Vec::new();
        let mut checks: Vec<Check> = Vec::new();
        if random.chance(20) {
            let wanted: Vec<i64> = (0..1 + random.below(30))
                .map(|_| random.below(21_100) as i64)
                .collect();
            for value in &wanted {
                conditions.push(eq("ID", &value.to_string()));
            }
            checks.push(Box::new(move |row| wanted.contains(&id(row))));
        }
        if random.chance(20) {
            let cases: Vec<String> = (0..1 + random.below(3))
                .map(|_| format!("jd{}", random.below(1_600)))
                .collect();
            let cases: Vec<&str> = cases.iter().map(String::as_str).collect();
            alternatives(
                &mut random,
                "Case Number",
                &cases,
                &mut conditions,
                &mut checks,
            );
        }
        if random.chance(40) {
            let from = 1 + random.below(10) as i64;
            let to = from + random.below(4) as i64;
            let hour = random.below(24) as i64;
            let start = CivilDateTime::from_parts(2020, 1, from, hour, 0, 0).expect("valid");
            let end = CivilDateTime::from_parts(2020, 1, to.min(10), 23, 59, 59).expect("valid");
            let start_text = format!("2020-01-{from:02} {hour:02}:00");
            let end_text = format!("2020-01-{:02}", to.min(10));
            conditions.push(range(&start_text, &end_text));
            checks.push(Box::new(move |row| start <= date(row) && date(row) <= end));
        }
        if random.chance(50) {
            alternatives(
                &mut random,
                "Primary Type",
                &TYPES,
                &mut conditions,
                &mut checks,
            );
        }
        if random.chance(40) {
            alternatives(
                &mut random,
                "District",
                &DISTRICTS,
                &mut conditions,
                &mut checks,
            );
        }
        if random.chance(40) {
            alternatives(
                &mut random,
                "Community Area",
                &AREAS,
                &mut conditions,
                &mut checks,
            );
        }
        if conditions.is_empty() {
            alternatives(
                &mut random,
                "Primary Type",
                &TYPES,
                &mut conditions,
                &mut checks,
            );
        }
        if random.chance(30) {
            alternatives(
                &mut random,
                "Location Description",
                &PLACES,
                &mut conditions,
                &mut checks,
            );
        }
        if random.chance(30) {
            alternatives(
                &mut random,
                "Arrest",
                &BOOLEANS,
                &mut conditions,
                &mut checks,
            );
        }

        let want = expected(&rows, |row| checks.iter().all(|check| check(row)));
        let query = query(&conditions, None);
        let (got, _, _) = results(&dataset, &query);
        assert_eq!(got, want, "round {round}: {conditions:?}");
        assert_eq!(
            count_of(&dataset, &query),
            want.len() as u64,
            "round {round}"
        );
        if !want.is_empty() {
            non_empty += 1;
        }
    }
    assert!(non_empty >= 20, "only {non_empty} rounds had results");
}

// Prefix conditions (startswith PRD).

fn sw(field: &str, prefix: &str) -> RawCondition {
    RawCondition::starts_with(field, prefix)
}

fn starts(row: &Row, name: &str, prefix: &str) -> bool {
    lower(row, name).starts_with(&prefix.to_lowercase())
}

#[test]
fn a_prefix_alone_on_an_indexed_field_finds_its_records() {
    let rows = fixture_rows();
    let fixture = Fixture::with_rows(&rows);
    let dataset = open(&fixture);

    for (name, prefix) in [
        ("Case Number", "JA1"),
        ("Case Number", "ja"),
        ("Case Number", "JA100"),
        ("Primary Type", "th"),
        ("Primary Type", "BAT"),
        ("Primary Type", "a"),
        ("District", "00"),
        ("District", "8"),
        ("District", "0"),
        ("District", "9"),
        ("Case Number", "zz"),
    ] {
        let conditions = [sw(name, prefix)];
        let want = expected(&rows, |row| starts(row, name, prefix));
        let (got, warnings, _) = results(&dataset, &query(&conditions, None));
        assert_eq!(got, want, "{name} {prefix}");
        assert!(warnings.is_empty(), "no warning for a prefix: {warnings:?}");
        assert_eq!(
            count_of(&dataset, &query(&conditions, None)),
            want.len() as u64
        );
    }
}

#[test]
fn overlapping_alternatives_on_one_field_return_each_record_once() {
    let rows = fixture_rows();
    let fixture = Fixture::with_rows(&rows);
    let dataset = open(&fixture);

    let conditions = [
        sw("Case Number", "ja"),
        sw("Case Number", "JA1"),
        eq("Case Number", "JA100"),
        eq("Case Number", "JA000"),
    ];
    let (got, warnings, returned) = results(&dataset, &query(&conditions, None));
    assert_eq!(got, expected(&rows, |_| true));
    assert_eq!(returned, rows.len() as u64);
    // The missing case number is still an equality alternative, and warned as typed (P4).
    assert_eq!(warnings, [messages::missing_case_number("JA000")]);

    let mixed = [
        eq("Primary Type", "THEFT"),
        sw("Primary Type", "BAT"),
        eq("District", ""),
        sw("District", "00"),
    ];
    let want = expected(&rows, |row| {
        (lower(row, "Primary Type") == "theft" || starts(row, "Primary Type", "bat"))
            && (col(row, "District").is_empty() || starts(row, "District", "00"))
    });
    assert_eq!(ids_of(&dataset, &mixed), want);
}

#[test]
fn a_prefix_is_intersected_with_the_other_indexed_conditions() {
    let rows = fixture_rows();
    let fixture = Fixture::with_rows(&rows);
    let dataset = open(&fixture);

    let cases: [(&[RawCondition], Check); 5] = [
        (
            &[eq("Primary Type", "THEFT"), sw("District", "00")],
            Box::new(|row| lower(row, "Primary Type") == "theft" && starts(row, "District", "00")),
        ),
        (
            &[
                sw("District", "00"),
                eq("ID", "10"),
                eq("ID", "40"),
                eq("ID", "5"),
            ],
            Box::new(|row| [10, 40, 5].contains(&id(row)) && starts(row, "District", "00")),
        ),
        (
            &[sw("Case Number", "JA1"), sw("Primary Type", "t")],
            Box::new(|row| starts(row, "Case Number", "ja1") && starts(row, "Primary Type", "t")),
        ),
        (
            &[range("2020-01-01", "2020-01-02"), sw("Case Number", "ja")],
            Box::new(|row| {
                at("01/01/2020 12:00:00 AM") <= date(row)
                    && date(row) <= at("01/02/2020 11:59:59 PM")
                    && starts(row, "Case Number", "ja")
            }),
        ),
        (
            &[
                eq("Community Area", "42"),
                sw("District", "0"),
                sw("Location Description", "VEH"),
            ],
            Box::new(|row| {
                col(row, "Community Area") == "42"
                    && starts(row, "District", "0")
                    && starts(row, "Location Description", "veh")
            }),
        ),
    ];
    for (conditions, predicate) in cases {
        let want = expected(&rows, predicate);
        assert_eq!(ids_of(&dataset, conditions), want, "{conditions:?}");
        assert_eq!(
            count_of(&dataset, &query(conditions, None)),
            want.len() as u64,
            "{conditions:?}"
        );
    }
}

#[test]
fn a_prefix_scan_crosses_the_blocks_of_the_index() {
    let rows = many_rows(3_000);
    let fixture = Fixture::with_rows(&rows);
    let dataset = open(&fixture);

    for prefix in ["jb", "JB1", "jb29", "jb3000", "jb30001", "ja", "jc"] {
        let want = expected(&rows, |row| starts(row, "Case Number", prefix));
        let conditions = [sw("Case Number", prefix)];
        assert_eq!(ids_of(&dataset, &conditions), want, "{prefix}");
        assert_eq!(
            count_of(&dataset, &query(&conditions, None)),
            want.len() as u64
        );
    }
    let limited = query(&[sw("Case Number", "jb")], Some(7));
    let (ids, _, returned) = results(&dataset, &limited);
    assert_eq!(returned, 7);
    assert_eq!(ids.len(), 7);
}

#[test]
fn the_summary_reports_prefixes_as_indexes_or_filters() {
    let fixture = Fixture::with_rows(&fixture_rows());
    let dataset = open(&fixture);

    let query = query(
        &[
            sw("District", "00"),
            sw("Location Description", "veh"),
            eq("Primary Type", "THEFT"),
            sw("IUCR", "03"),
        ],
        None,
    );
    let summary = Execution::start(&dataset, &query, &never)
        .expect("starts")
        .count()
        .expect("no read fails");
    assert_eq!(summary.indexes, [Field::PrimaryType, Field::District]);
    assert_eq!(summary.filters, [Field::Iucr, Field::LocationDescription]);
}

/// Prefixes of the values of `domain`, from one character to the whole value, in mixed case.
fn prefixes_of(random: &mut Random, domain: &[&str]) -> String {
    let value = random.pick(domain);
    let cut = 1 + random.below(value.len().max(1));
    let prefix: String = value.chars().take(cut).collect();
    if random.chance(50) {
        prefix.to_uppercase()
    } else {
        prefix
    }
}

/// Up to two values and up to two prefixes of `field`, at least one of either.
fn prefix_alternatives(
    random: &mut Random,
    field: &'static str,
    domain: &[&str],
    conditions: &mut Vec<RawCondition>,
    checks: &mut Vec<Check>,
) {
    let values: Vec<String> = (0..random.below(3))
        .map(|_| random.pick(domain).to_owned())
        .collect();
    let mut prefixes: Vec<String> = (0..random.below(3))
        .map(|_| prefixes_of(random, domain))
        .filter(|prefix| !prefix.is_empty())
        .collect();
    if values.is_empty() && prefixes.is_empty() {
        prefixes.push(prefixes_of(random, domain));
        prefixes.retain(|prefix| !prefix.is_empty());
        if prefixes.is_empty() {
            prefixes.push("0".to_owned());
        }
    }
    for value in &values {
        conditions.push(eq(field, value));
    }
    for prefix in &prefixes {
        conditions.push(sw(field, prefix));
    }
    let values: Vec<String> = values.iter().map(|value| value.to_lowercase()).collect();
    let prefixes: Vec<String> = prefixes
        .iter()
        .map(|prefix| prefix.to_lowercase())
        .collect();
    checks.push(Box::new(move |row| {
        let actual = lower(row, field);
        values.contains(&actual)
            || (!actual.is_empty() && prefixes.iter().any(|prefix| actual.starts_with(prefix)))
    }));
}

#[test]
fn the_plan_with_prefixes_returns_what_a_brute_force_filter_of_the_rows_returns() {
    let mut random = Random(0x2545_f491_4f6c_dd1d);
    let rows = random_rows(&mut random, 3_000);
    let fixture = Fixture::with_rows(&rows);
    let dataset = open(&fixture);
    let cases: Vec<String> = (0..40).map(|n| format!("JD{}", n * 37)).collect();
    let cases: Vec<&str> = cases.iter().map(String::as_str).collect();
    let mut non_empty = 0;

    for round in 0..80 {
        let mut conditions = Vec::new();
        let mut checks: Vec<Check> = Vec::new();
        if random.chance(15) {
            let wanted: Vec<i64> = (0..1 + random.below(300))
                .map(|_| random.below(21_100) as i64)
                .collect();
            for value in &wanted {
                conditions.push(eq("ID", &value.to_string()));
            }
            checks.push(Box::new(move |row| wanted.contains(&id(row))));
        }
        if random.chance(25) {
            let day = 1 + random.below(10) as i64;
            let start = CivilDateTime::from_parts(2020, 1, day, 0, 0, 0).expect("valid");
            let end = CivilDateTime::from_parts(2020, 1, day, 23, 59, 59).expect("valid");
            conditions.push(range(
                &format!("2020-01-{day:02}"),
                &format!("2020-01-{day:02}"),
            ));
            checks.push(Box::new(move |row| start <= date(row) && date(row) <= end));
        }
        for (field, domain, percent) in [
            ("Case Number", &cases[..], 30),
            ("Primary Type", &TYPES[..], 50),
            ("District", &DISTRICTS[..], 40),
            ("Location Description", &PLACES[..], 25),
        ] {
            if random.chance(percent) {
                prefix_alternatives(&mut random, field, domain, &mut conditions, &mut checks);
            }
        }
        if random.chance(30) {
            alternatives(
                &mut random,
                "Community Area",
                &AREAS,
                &mut conditions,
                &mut checks,
            );
        }
        if Query::build(&conditions).is_err() {
            prefix_alternatives(
                &mut random,
                "Primary Type",
                &TYPES,
                &mut conditions,
                &mut checks,
            );
        }

        let want = expected(&rows, |row| checks.iter().all(|check| check(row)));
        let query = query(&conditions, None);
        let (got, _, _) = results(&dataset, &query);
        assert_eq!(got, want, "round {round}: {conditions:?}");
        assert_eq!(
            count_of(&dataset, &query),
            want.len() as u64,
            "round {round}"
        );
        if !want.is_empty() {
            non_empty += 1;
        }
    }
    assert!(non_empty >= 20, "only {non_empty} rounds found records");
}
