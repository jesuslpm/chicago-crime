//! Every check of the verification, in the order they are reported:
//!
//! | Criterion | What it proves |
//! |---|---|
//! | `LAYOUT` | `CURRENT` names a version folder that holds exactly the seven dataset files. |
//! | `MANIFEST-SCHEMA` | The manifest is JSON with schema version 1, a known compression and the five indexes. |
//! | `CSV` | The CSV has the expected header and every row can be read. |
//! | `MANIFEST-SOURCE` | The manifest names the CSV by file name, size and SHA-256, all computed here again. |
//! | `MANIFEST-TABLES` | The manifest's entry counts and file sizes equal the tables on disk. |
//! | `AC-G1` | The primary table has one entry per valid record, keys strictly increasing. |
//! | `AC-G3` | Every stored record equals its CSV row field by field. |
//! | `AC-G3-SAMPLE` | The parent's AC3 sample: first and last ID, one without coordinates, one without Ward. A category the CSV lacks is skipped, unless `--reference`. |
//! | `AC-G3-DST` | Rows in the spring-forward hour are stored exactly as published (parent AC12). SKIP when the CSV has none, unless `--reference`. |
//! | `AC-G4-<index>` | Each index table equals, entry by entry, the keys rebuilt from the CSV. |
//! | `AC-G4-THEFT`, `AC-G4-COMMUNITY-NIL` | The parent's AC4 and AC5 counts. |
//! | `AC-G4-SHARED-CASES` | Every shared case number lists every ID that shares it (parent AC8). |
//!
//! The CSV is read once, while its SHA-256 is computed. A row is compared with the stored record
//! of its ID the first time that ID appears, as the generator keeps the first occurrence
//! (decision D5); rows whose ID is not stored, or appears again, are the rejected ones. A CSV
//! whose rejected row shares its ID with a later valid row would therefore be reported as a
//! mismatch: never a false PASS. The reference CSV has neither rejected rows nor repeated IDs.
//!
//! Without [`Options::reference`] the expected figures come from the CSV itself (parent PRD:
//! "con otro CSV compatible, esos resultados deben coincidir con los registros válidos realmente
//! procesados"), and a compatible CSV may lack a sample category altogether: no row in the
//! spring-forward hour, without coordinates or without Ward. That sample is then skipped (SKIP,
//! neither PASS nor FAIL), while every row is still compared field by field in `AC-G3`. With
//! [`Options::reference`], the exact figures measured on the reference CSV are required too, and
//! so is every sample category, since the reference CSV has rows of each.

use std::collections::{BTreeMap, HashSet};
use std::fmt::Write as _;
use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

use csv::ByteRecord;
use foundationdb_tuple::Element;
use sha2::{Digest, Sha256};

use crate::civil;
use crate::dataset::{self, MANIFEST_FILE, ManifestFacts, PRIMARY_FILE, PrimaryTable};
use crate::fields::{self, HEADER};
use crate::indexes::{self, INDEXES, IndexSpec};
use crate::report::{Check, Report, thousands};

/// Figures measured on the reference CSV (parent PRD, "Estado de partida" and decision D3; child
/// AC-G1 and AC-G4).
pub mod reference {
    pub const VALID_RECORDS: u64 = 8_656_904;
    pub const REJECTED: u64 = 0;
    pub const THEFT: u64 = 1_838_502;
    pub const COMMUNITY_AREA_NIL: u64 = 613_720;
    /// Records that repeat a case number already seen.
    pub const CASE_NUMBER_REPEATS: u64 = 632;
    pub const SPRING_FORWARD_ROWS: u64 = 253;
}

/// What the verification expects besides the CSV itself.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Options {
    /// Also require the exact figures that the PRDs measured on the reference CSV.
    pub reference: bool,
}

const SCHEMA_VERSION: u64 = 1;
const COMPRESSIONS: [&str; 2] = ["snappy", "none"];
/// Mismatches listed under a failed check.
const MAX_EXAMPLES: usize = 5;
/// Large reads keep the scan of a 2.3 GB CSV cheap.
const READ_BUFFER_BYTES: usize = 1 << 20;
const UTF8_BOM: &str = "\u{feff}";

/// Verifies the dataset published in the output folder `dataset` against the CSV `csv`.
/// `progress` receives a short line before each long step.
pub fn verify(
    csv: &Path,
    dataset: &Path,
    options: Options,
    progress: &mut dyn FnMut(&str),
) -> Report {
    let mut report = Report::default();
    progress("resolving CURRENT");
    let version = match dataset::resolve_current(dataset) {
        Ok(version) => version,
        Err(error) => {
            report.checks.push(Check::new("LAYOUT", false, error));
            return report;
        }
    };
    report.checks.push(layout_check(&version));
    let manifest = dataset::read_manifest(&version.join(MANIFEST_FILE));
    report.checks.push(schema_check(&manifest));
    let Ok(manifest) = manifest else {
        return report;
    };

    progress("loading primary.sst");
    let primary = match PrimaryTable::load(&version.join(PRIMARY_FILE)) {
        Ok(primary) => primary,
        Err(error) => {
            report.checks.push(Check::new("AC-G1", false, error));
            return report;
        }
    };
    progress("reading the CSV and comparing every row with primary.sst");
    let mut scan = match scan_csv(csv, &primary) {
        Ok(scan) => scan,
        Err(error) => {
            report.checks.push(Check::new("CSV", false, error));
            return report;
        }
    };
    let stored = StoredFacts::of(&primary, &scan);
    drop(primary);

    progress("sorting the index keys rebuilt from the CSV");
    std::thread::scope(|scope| {
        for keys in &mut scan.expected_keys {
            scope.spawn(|| keys.sort_unstable());
        }
    });
    let mut index_scans = Vec::with_capacity(INDEXES.len());
    for (spec, expected) in INDEXES.iter().zip(&scan.expected_keys) {
        progress(&format!("checking {}", spec.file));
        index_scans.push(scan_index(spec, &version.join(spec.file), expected));
    }
    let expected_shared = indexes::shared_case_numbers(
        scan.expected_keys[index_position("case_number")]
            .iter()
            .map(Vec::as_slice),
    );

    report.checks.extend([
        csv_check(&scan),
        source_check(csv, &scan, &manifest),
        tables_check(&version, &manifest, &stored, &index_scans),
        ac_g1_check(&manifest, &stored, &scan, options),
        ac_g3_check(&manifest, &stored, &scan),
        sample_check(&stored, &scan, options),
        dst_check(&scan, options),
    ]);
    for ((spec, index_scan), untypable) in INDEXES.iter().zip(&index_scans).zip(&scan.untypable) {
        report
            .checks
            .push(index_check(spec, index_scan, *untypable, &stored));
    }
    report.checks.extend([
        theft_check(&index_scans, &scan, options),
        community_nil_check(&index_scans, &scan, options),
        shared_cases_check(&index_scans, expected_shared, options),
    ]);
    report
}

fn index_position(name: &str) -> usize {
    INDEXES
        .iter()
        .position(|spec| spec.name == name)
        .unwrap_or_else(|| panic!("{name} is an index"))
}

// --- Layout and manifest ---

fn layout_check(version: &Path) -> Check {
    let name = version
        .file_name()
        .map_or_else(String::new, |name| name.to_string_lossy().into_owned());
    let expected = dataset::expected_files();
    match dataset::folder_contents(version) {
        Ok(found) if found == expected => Check::new(
            "LAYOUT",
            true,
            format!(
                "CURRENT names {name}, which holds exactly the {} dataset files",
                expected.len()
            ),
        ),
        Ok(found) => Check::new(
            "LAYOUT",
            false,
            format!("{name} holds {found:?}; expected exactly {expected:?}"),
        ),
        Err(error) => Check::new("LAYOUT", false, error),
    }
}

fn schema_check(manifest: &Result<ManifestFacts, String>) -> Check {
    let manifest = match manifest {
        Ok(manifest) => manifest,
        Err(error) => return Check::new("MANIFEST-SCHEMA", false, error.clone()),
    };
    let tables: Vec<(&str, &str)> = manifest
        .indexes
        .iter()
        .map(|(name, table)| (name.as_str(), table.file.as_str()))
        .collect();
    let expected_tables: Vec<(&str, &str)> =
        INDEXES.iter().map(|spec| (spec.name, spec.file)).collect();
    let mut details = Vec::new();
    if manifest.schema_version != SCHEMA_VERSION {
        details.push(format!("the schema version must be {SCHEMA_VERSION}"));
    }
    if !COMPRESSIONS.contains(&manifest.compression.as_str()) {
        details.push(format!("the compression must be one of {COMPRESSIONS:?}"));
    }
    if manifest.primary.file != PRIMARY_FILE {
        details.push(format!(
            "the primary table is {:?}, expected {PRIMARY_FILE:?}",
            manifest.primary.file
        ));
    }
    if tables != expected_tables {
        details.push(format!(
            "the indexes are {tables:?}, expected {expected_tables:?}"
        ));
    }
    let names: Vec<&str> = tables.iter().map(|(name, _)| *name).collect();
    Check::new(
        "MANIFEST-SCHEMA",
        details.is_empty(),
        format!(
            "schema_version {}, compression {}, generated_at {}, indexes {}",
            manifest.schema_version,
            manifest.compression,
            manifest.generated_at,
            names.join(", ")
        ),
    )
    .with_details(details)
}

fn source_check(csv: &Path, scan: &CsvScan, manifest: &ManifestFacts) -> Check {
    let file_name = csv
        .file_name()
        .map_or_else(String::new, |name| name.to_string_lossy().into_owned());
    let mut details = Vec::new();
    if manifest.source_file_name != file_name {
        details.push(format!(
            "file name: manifest {:?}, CSV {file_name:?}",
            manifest.source_file_name
        ));
    }
    if manifest.source_size_bytes != scan.size_bytes {
        details.push(format!(
            "size: manifest {} bytes, CSV {} bytes",
            thousands(manifest.source_size_bytes),
            thousands(scan.size_bytes)
        ));
    }
    if manifest.source_sha256 != scan.sha256 {
        details.push(format!(
            "SHA-256: manifest {}, CSV {}",
            manifest.source_sha256, scan.sha256
        ));
    }
    Check::new(
        "MANIFEST-SOURCE",
        details.is_empty(),
        format!(
            "CSV {file_name}: {} bytes, SHA-256 {} computed here; manifest source {}, {} bytes, \
             SHA-256 {}",
            thousands(scan.size_bytes),
            scan.sha256,
            manifest.source_file_name,
            thousands(manifest.source_size_bytes),
            manifest.source_sha256
        ),
    )
    .with_details(details)
}

fn tables_check(
    version: &Path,
    manifest: &ManifestFacts,
    stored: &StoredFacts,
    index_scans: &[IndexScan],
) -> Check {
    let mut details = Vec::new();
    let mut compare =
        |file: &str, manifest_entries: u64, manifest_size: u64, entries: Option<u64>| {
            match dataset::file_size(&version.join(file)) {
                Ok(size) if size == manifest_size => {}
                Ok(size) => details.push(format!(
                    "{file}: manifest size {} bytes, on disk {} bytes",
                    thousands(manifest_size),
                    thousands(size)
                )),
                Err(error) => details.push(error),
            }
            if entries != Some(manifest_entries) {
                details.push(format!(
                    "{file}: manifest {} entries, table {}",
                    thousands(manifest_entries),
                    entries.map_or_else(|| "unreadable".to_owned(), thousands)
                ));
            }
        };
    compare(
        PRIMARY_FILE,
        manifest.primary.entries,
        manifest.primary.size_bytes,
        Some(stored.entries),
    );
    for ((_, table), index_scan) in manifest.indexes.iter().zip(index_scans) {
        compare(
            &table.file,
            table.entries,
            table.size_bytes,
            index_scan.error.is_none().then_some(index_scan.entries),
        );
    }
    Check::new(
        "MANIFEST-TABLES",
        details.is_empty() && manifest.indexes.len() == index_scans.len(),
        format!(
            "entries and file sizes of {PRIMARY_FILE} and {} index tables in the manifest \
             compared with the files on disk",
            manifest.indexes.len()
        ),
    )
    .with_details(details)
}

// --- The CSV pass ---

/// Hashes and counts every byte that is read through it.
struct HashingReader<R> {
    inner: R,
    hasher: Sha256,
    bytes: u64,
}

impl<R: Read> Read for HashingReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let read = self.inner.read(buf)?;
        self.hasher.update(&buf[..read]);
        self.bytes += read as u64;
        Ok(read)
    }
}

/// Everything learnt from the one pass over the CSV.
#[derive(Debug, Default)]
struct CsvScan {
    size_bytes: u64,
    sha256: String,
    data_rows: u64,
    /// Rows compared with the stored record of their ID: the first row of each stored ID.
    compared: u64,
    /// Stored records whose CSV row has been compared, by position in the primary table.
    matched: Vec<bool>,
    mismatched_rows: u64,
    mismatched_ids: HashSet<i64>,
    mismatch_examples: Vec<String>,
    /// Rows that are not stored: no valid ID, or an ID that the primary table does not hold.
    not_stored: u64,
    /// Rows whose ID appeared in an earlier row.
    repeated: u64,
    spring_forward_rows: u64,
    spring_forward_exact: u64,
    spring_forward_example: Option<(i64, String)>,
    spring_forward_failures: Vec<String>,
    first_without_coordinates: Option<i64>,
    first_without_ward: Option<i64>,
    /// Per index, in [`INDEXES`] order: the keys of every compared row.
    expected_keys: Vec<Vec<Vec<u8>>>,
    /// Per index: compared rows whose indexed field cannot be typed.
    untypable: Vec<u64>,
    theft_rows: u64,
    community_area_empty_rows: u64,
}

fn scan_csv(path: &Path, primary: &PrimaryTable) -> Result<CsvScan, String> {
    let unreadable =
        |error: &dyn std::fmt::Display| format!("{} cannot be read: {error}", path.display());
    let file = File::open(path).map_err(|error| unreadable(&error))?;
    let mut hashed = HashingReader {
        inner: file,
        hasher: Sha256::new(),
        bytes: 0,
    };
    let mut scan = CsvScan {
        matched: vec![false; primary.len()],
        expected_keys: INDEXES
            .iter()
            .map(|_| Vec::with_capacity(primary.len()))
            .collect(),
        untypable: vec![0; INDEXES.len()],
        ..CsvScan::default()
    };
    {
        let mut reader = csv::ReaderBuilder::new()
            .has_headers(false)
            .flexible(true)
            .buffer_capacity(READ_BUFFER_BYTES)
            .from_reader(&mut hashed);
        let mut record = ByteRecord::new();
        if !reader
            .read_byte_record(&mut record)
            .map_err(|error| unreadable(&error))?
        {
            return Err(format!("{} is empty", path.display()));
        }
        check_header(&record)?;
        while reader
            .read_byte_record(&mut record)
            .map_err(|error| unreadable(&error))?
        {
            scan.data_rows += 1;
            scan.row(&record, primary);
        }
    }
    // Whatever the parser left unread still belongs to the file.
    io::copy(&mut hashed, &mut io::sink()).map_err(|error| unreadable(&error))?;
    let digest = hashed.hasher.finalize();
    for byte in digest.iter() {
        let _ = write!(scan.sha256, "{byte:02x}");
    }
    scan.size_bytes = hashed.bytes;
    Ok(scan)
}

fn check_header(record: &ByteRecord) -> Result<(), String> {
    let found: Vec<String> = record
        .iter()
        .enumerate()
        .map(|(column, field)| {
            let text = String::from_utf8_lossy(field);
            if column == 0 {
                text.trim_start_matches(UTF8_BOM).to_owned()
            } else {
                text.into_owned()
            }
        })
        .collect();
    if found == HEADER {
        Ok(())
    } else {
        Err(format!("the header is {found:?}; expected {HEADER:?}"))
    }
}

impl CsvScan {
    /// One data row: compared with its stored record the first time its ID appears.
    fn row(&mut self, record: &ByteRecord, primary: &PrimaryTable) {
        let fields: Option<Vec<&str>> = (record.len() == HEADER.len())
            .then(|| {
                record
                    .iter()
                    .map(|field| std::str::from_utf8(field).ok())
                    .collect()
            })
            .flatten();
        let Some(fields) = fields else {
            self.not_stored += 1;
            return;
        };
        let Some((id, position)) = fields[0]
            .parse::<i64>()
            .ok()
            .and_then(|id| primary.find(id).map(|position| (id, position)))
        else {
            self.not_stored += 1;
            return;
        };
        if self.matched[position] {
            self.repeated += 1;
            return;
        }
        self.matched[position] = true;
        self.compared += 1;

        let (problems, date_equal) = match fields::decode_value(primary.value(position)) {
            Ok(stored) => {
                let mismatches = fields::compare_row(&fields, &stored);
                let date_equal = mismatches.iter().all(|mismatch| mismatch.column != "Date");
                let problems: Vec<String> = mismatches
                    .into_iter()
                    .map(|mismatch| {
                        format!(
                            "{}: CSV {:?}, stored {}",
                            mismatch.column, mismatch.csv, mismatch.stored
                        )
                    })
                    .collect();
                (problems, date_equal)
            }
            Err(error) => (vec![error], false),
        };
        if !problems.is_empty() {
            self.mismatched_rows += 1;
            self.mismatched_ids.insert(id);
            if self.mismatch_examples.len() < MAX_EXAMPLES {
                self.mismatch_examples
                    .push(format!("ID {id}: {}", problems.join("; ")));
            }
        }
        self.spring_forward(id, fields[column::DATE], date_equal);
        self.sample_candidates(id, &fields);
        self.index_keys(id, &fields);
    }

    /// Rows whose `Date` falls in the hour skipped when daylight saving time starts.
    fn spring_forward(&mut self, id: i64, date: &str, date_equal: bool) {
        let in_gap = civil::parse_civil(date).is_some_and(civil::is_spring_forward_gap);
        if !in_gap {
            return;
        }
        self.spring_forward_rows += 1;
        if date_equal {
            self.spring_forward_exact += 1;
            self.spring_forward_example
                .get_or_insert_with(|| (id, date.to_owned()));
        } else if self.spring_forward_failures.len() < MAX_EXAMPLES {
            self.spring_forward_failures
                .push(format!("ID {id}: {date}"));
        }
    }

    /// The first rows without coordinates and without Ward, for the AC3 sample.
    fn sample_candidates(&mut self, id: i64, fields: &[&str]) {
        if fields[column::X_COORDINATE].is_empty() && fields[column::LATITUDE].is_empty() {
            self.first_without_coordinates.get_or_insert(id);
        }
        if fields[column::WARD].is_empty() {
            self.first_without_ward.get_or_insert(id);
        }
    }

    fn index_keys(&mut self, id: i64, fields: &[&str]) {
        for (position, spec) in INDEXES.iter().enumerate() {
            match indexes::expected_key(spec, fields, id) {
                Some(key) => self.expected_keys[position].push(key),
                None => self.untypable[position] += 1,
            }
        }
        if fields[column::PRIMARY_TYPE] == "THEFT" {
            self.theft_rows += 1;
        }
        if fields[column::COMMUNITY_AREA].is_empty() {
            self.community_area_empty_rows += 1;
        }
    }
}

/// Positions in [`HEADER`] of the columns the checks look at directly.
mod column {
    pub const DATE: usize = 2;
    pub const PRIMARY_TYPE: usize = 5;
    pub const WARD: usize = 12;
    pub const COMMUNITY_AREA: usize = 13;
    pub const X_COORDINATE: usize = 15;
    pub const LATITUDE: usize = 19;
}

/// What remains of the primary table once the CSV pass is done.
struct StoredFacts {
    entries: u64,
    /// The first and the last ID of the table, and whether a CSV row was compared with each.
    first_id: Option<(i64, bool)>,
    last_id: Option<(i64, bool)>,
    order_violation: Option<String>,
    /// Stored records that no CSV row matched.
    without_row: u64,
}

impl StoredFacts {
    fn of(primary: &PrimaryTable, scan: &CsvScan) -> Self {
        let entries = primary.len() as u64;
        let at = |position: usize| (primary.id(position), scan.matched[position]);
        Self {
            entries,
            first_id: (!primary.is_empty()).then(|| at(0)),
            last_id: primary.len().checked_sub(1).map(at),
            order_violation: primary.order_violation.clone(),
            without_row: entries - scan.compared,
        }
    }
}

fn csv_check(scan: &CsvScan) -> Check {
    Check::new(
        "CSV",
        true,
        format!(
            "the header has the {} expected columns; {} data rows read ({} bytes)",
            HEADER.len(),
            thousands(scan.data_rows),
            thousands(scan.size_bytes)
        ),
    )
}

// --- AC-G1 and AC-G3 ---

fn ac_g1_check(
    manifest: &ManifestFacts,
    stored: &StoredFacts,
    scan: &CsvScan,
    options: Options,
) -> Check {
    let mut details = Vec::new();
    if let Some(violation) = &stored.order_violation {
        details.push(violation.clone());
    }
    if stored.entries != manifest.valid {
        details.push(format!(
            "{} entries, manifest valid {}",
            thousands(stored.entries),
            thousands(manifest.valid)
        ));
    }
    if manifest.valid + manifest.rejected != scan.data_rows {
        details.push(format!(
            "manifest valid {} + rejected {} differ from the {} CSV rows",
            thousands(manifest.valid),
            thousands(manifest.rejected),
            thousands(scan.data_rows)
        ));
    }
    if options.reference
        && (stored.entries != reference::VALID_RECORDS || manifest.rejected != reference::REJECTED)
    {
        details.push(format!(
            "the reference CSV gives {} entries and {} rejected",
            thousands(reference::VALID_RECORDS),
            thousands(reference::REJECTED)
        ));
    }
    let range = match (stored.first_id, stored.last_id) {
        (Some((first, _)), Some((last, _))) => format!("IDs {first}..{last}"),
        _ => "no IDs".to_owned(),
    };
    Check::new(
        "AC-G1",
        details.is_empty(),
        format!(
            "{PRIMARY_FILE}: {} entries in strictly increasing ID order ({range}) = manifest \
             valid {} = {} CSV rows - {} rejected",
            thousands(stored.entries),
            thousands(manifest.valid),
            thousands(scan.data_rows),
            thousands(manifest.rejected)
        ),
    )
    .with_details(details)
}

/// Every stored record against its CSV row. The reference count needs no check of its own here:
/// AC-G1 pins the number of stored records, and every one of them must have been compared.
fn ac_g3_check(manifest: &ManifestFacts, stored: &StoredFacts, scan: &CsvScan) -> Check {
    let not_stored = scan.not_stored + scan.repeated;
    let mut details = scan.mismatch_examples.clone();
    if stored.without_row > 0 {
        details.push(format!(
            "{} stored records have no CSV row with their ID",
            thousands(stored.without_row)
        ));
    }
    if not_stored != manifest.rejected {
        details.push(format!(
            "{} CSV rows are not stored ({} without a stored ID, {} repeated IDs), but the \
             manifest says {} rejected",
            thousands(not_stored),
            thousands(scan.not_stored),
            thousands(scan.repeated),
            thousands(manifest.rejected)
        ));
    }
    Check::new(
        "AC-G3",
        scan.mismatched_rows == 0 && details.is_empty(),
        format!(
            "{} CSV rows compared field by field with {PRIMARY_FILE}: {} mismatched rows; {} \
             stored records without a CSV row; {} CSV rows not stored = manifest rejected {}",
            thousands(scan.compared),
            thousands(scan.mismatched_rows),
            thousands(stored.without_row),
            thousands(not_stored),
            thousands(manifest.rejected)
        ),
    )
    .with_details(details)
}

/// A sample whose category the CSV lacks is skipped, unless the reference CSV is required: that
/// one has a row of every category.
fn sample_check(stored: &StoredFacts, scan: &CsvScan, options: Options) -> Check {
    const CRITERION: &str = "AC-G3-SAMPLE";
    // The rows without coordinates or Ward were taken from compared rows only.
    let compared = |id: Option<i64>| id.map(|id| (id, true));
    let samples = [
        ("first ID", stored.first_id),
        ("last ID", stored.last_id),
        (
            "without coordinates",
            compared(scan.first_without_coordinates),
        ),
        ("without Ward", compared(scan.first_without_ward)),
    ];
    let mut passed = true;
    let mut skipped = 0;
    let described: Vec<String> = samples
        .iter()
        .map(|&(label, sample)| match sample {
            Some((id, was_compared)) => {
                let equal = was_compared && !scan.mismatched_ids.contains(&id);
                passed &= equal;
                let outcome = match (was_compared, equal) {
                    (false, _) => "has no CSV row",
                    (true, true) => "equal",
                    (true, false) => "DIFFERENT",
                };
                format!("{label} {id} {outcome}")
            }
            None if !options.reference => {
                skipped += 1;
                format!("{label}: SKIP, the CSV has no such row")
            }
            None => {
                passed = false;
                format!("{label}: no such row")
            }
        })
        .collect();
    let summary = format!(
        "parent AC3 sample, every field compared: {}",
        described.join(", ")
    );
    if skipped == samples.len() {
        Check::skipped(CRITERION, summary)
    } else {
        Check::new(CRITERION, passed, summary)
    }
}

/// Without the reference CSV, a CSV with no row in the spring-forward hour skips the check.
fn dst_check(scan: &CsvScan, options: Options) -> Check {
    if scan.spring_forward_rows == 0 && !options.reference {
        return Check::skipped(
            "AC-G3-DST",
            "the CSV has no row with a Date in the spring-forward hour",
        );
    }
    let mut passed =
        scan.spring_forward_rows > 0 && scan.spring_forward_exact == scan.spring_forward_rows;
    let mut details = scan.spring_forward_failures.clone();
    if options.reference && scan.spring_forward_rows != reference::SPRING_FORWARD_ROWS {
        passed = false;
        details.push(format!(
            "the reference CSV has {} such rows",
            reference::SPRING_FORWARD_ROWS
        ));
    }
    let example = scan
        .spring_forward_example
        .as_ref()
        .map_or_else(String::new, |(id, date)| {
            format!(", e.g. ID {id} with Date {date}")
        });
    Check::new(
        "AC-G3-DST",
        passed,
        format!(
            "{} rows with a Date in the spring-forward hour, {} stored exactly as published{example}",
            thousands(scan.spring_forward_rows),
            thousands(scan.spring_forward_exact)
        ),
    )
    .with_details(details)
}

// --- AC-G4 ---

/// What one index table holds, compared with the keys rebuilt from the CSV.
#[derive(Debug, Default)]
struct IndexScan {
    /// The table could not be read.
    error: Option<String>,
    entries: u64,
    expected_entries: u64,
    order_violation: Option<String>,
    non_empty_values: u64,
    undecodable_keys: u64,
    first_difference: Option<String>,
    nil_values: u64,
    theft_values: u64,
    /// Only for the case number index.
    shared: Option<Result<BTreeMap<String, Vec<i64>>, String>>,
}

fn scan_index(spec: &IndexSpec, path: &Path, expected: &[Vec<u8>]) -> IndexScan {
    let mut scan = IndexScan {
        expected_entries: expected.len() as u64,
        ..IndexScan::default()
    };
    let entries = match dataset::read_entries(path) {
        Ok(entries) => entries,
        Err(error) => {
            scan.error = Some(error);
            return scan;
        }
    };
    scan.entries = entries.len() as u64;
    for (position, (key, value)) in entries.iter().enumerate() {
        if position > 0 && scan.order_violation.is_none() && *key <= entries[position - 1].0 {
            scan.order_violation = Some(format!(
                "entry {position}: {} does not follow {}",
                describe_key(key),
                describe_key(&entries[position - 1].0)
            ));
        }
        if !value.is_empty() {
            scan.non_empty_values += 1;
        }
        if scan.first_difference.is_none() && expected.get(position) != Some(key) {
            scan.first_difference = Some(format!(
                "entry {position}: expected {}, found {}",
                expected
                    .get(position)
                    .map_or_else(|| "no more entries".to_owned(), |key| describe_key(key)),
                describe_key(key)
            ));
        }
        match indexes::decode_key(key) {
            Some((Element::Nil, _)) => scan.nil_values += 1,
            Some((Element::String(text), _)) if text == "THEFT" => scan.theft_values += 1,
            Some(_) => {}
            None => scan.undecodable_keys += 1,
        }
    }
    if scan.first_difference.is_none() && entries.len() < expected.len() {
        scan.first_difference = Some(format!(
            "entry {}: expected {}, found the end of the table",
            entries.len(),
            describe_key(&expected[entries.len()])
        ));
    }
    if spec.name == "case_number" {
        scan.shared = Some(indexes::shared_case_numbers(
            entries.iter().map(|(key, _)| key.as_slice()),
        ));
    }
    scan
}

/// An index key as `(value, id)`, or its bytes when it does not decode.
fn describe_key(key: &[u8]) -> String {
    match indexes::decode_key(key) {
        Some((value, id)) => format!("({value:?}, {id})"),
        None => format!("{key:02x?}"),
    }
}

fn index_check(spec: &IndexSpec, scan: &IndexScan, untypable: u64, stored: &StoredFacts) -> Check {
    let criterion = format!("AC-G4-{}", spec.name);
    if let Some(error) = &scan.error {
        return Check::new(criterion, false, error.clone());
    }
    let mut details = Vec::new();
    if scan.entries != stored.entries {
        details.push(format!(
            "{} entries, but {} valid records",
            thousands(scan.entries),
            thousands(stored.entries)
        ));
    }
    details.extend(scan.order_violation.clone());
    details.extend(scan.first_difference.clone());
    if scan.non_empty_values > 0 {
        details.push(format!(
            "{} entries have a non-empty value",
            thousands(scan.non_empty_values)
        ));
    }
    if scan.undecodable_keys > 0 {
        details.push(format!(
            "{} keys are not (value, id) tuples",
            thousands(scan.undecodable_keys)
        ));
    }
    if untypable > 0 {
        details.push(format!(
            "{} stored rows have a {} field that cannot be typed",
            thousands(untypable),
            HEADER[spec.column]
        ));
    }
    Check::new(
        criterion,
        details.is_empty(),
        format!(
            "{}: {} entries = {} valid records; keys strictly increasing; equal entry by entry \
             to the {} (value or Nil, id) keys rebuilt from the CSV",
            spec.file,
            thousands(scan.entries),
            thousands(stored.entries),
            thousands(scan.expected_entries)
        ),
    )
    .with_details(details)
}

fn count_check(
    criterion: &str,
    what: &str,
    in_table: Option<u64>,
    in_csv: u64,
    reference: Option<u64>,
) -> Check {
    let table_text = in_table.map_or_else(|| "unreadable".to_owned(), thousands);
    let mut passed = in_table == Some(in_csv);
    let mut summary = format!(
        "{what}: {table_text} entries in the index, {} rows in the CSV",
        thousands(in_csv)
    );
    if let Some(expected) = reference {
        passed &= in_csv == expected;
        let _ = write!(
            summary,
            ", {} expected on the reference CSV",
            thousands(expected)
        );
    }
    Check::new(criterion, passed, summary)
}

fn theft_check(index_scans: &[IndexScan], scan: &CsvScan, options: Options) -> Check {
    let table = &index_scans[index_position("primary_type")];
    count_check(
        "AC-G4-THEFT",
        "primary type THEFT",
        table.error.is_none().then_some(table.theft_values),
        scan.theft_rows,
        options.reference.then_some(reference::THEFT),
    )
}

fn community_nil_check(index_scans: &[IndexScan], scan: &CsvScan, options: Options) -> Check {
    let table = &index_scans[index_position("community_area")];
    count_check(
        "AC-G4-COMMUNITY-NIL",
        "community area without value (Nil)",
        table.error.is_none().then_some(table.nil_values),
        scan.community_area_empty_rows,
        options.reference.then_some(reference::COMMUNITY_AREA_NIL),
    )
}

fn shared_cases_check(
    index_scans: &[IndexScan],
    expected: Result<BTreeMap<String, Vec<i64>>, String>,
    options: Options,
) -> Check {
    const CRITERION: &str = "AC-G4-SHARED-CASES";
    let table = &index_scans[index_position("case_number")];
    let found = match (&table.error, &table.shared) {
        (Some(error), _) => return Check::new(CRITERION, false, error.clone()),
        (None, Some(Ok(found))) => found,
        (None, Some(Err(error))) => return Check::new(CRITERION, false, error.clone()),
        (None, None) => return Check::new(CRITERION, false, "the case number index was not read"),
    };
    let expected = match expected {
        Ok(expected) => expected,
        Err(error) => return Check::new(CRITERION, false, error),
    };
    let ids: u64 = expected.values().map(|ids| ids.len() as u64).sum();
    let numbers = expected.len() as u64;
    let repeats = ids - numbers;
    let mut details: Vec<String> = expected
        .iter()
        .filter(|(case_number, ids)| found.get(*case_number) != Some(ids))
        .take(MAX_EXAMPLES)
        .map(|(case_number, ids)| {
            format!(
                "{case_number}: CSV IDs {ids:?}, index IDs {:?}",
                found.get(case_number)
            )
        })
        .collect();
    if found.len() != expected.len() {
        details.push(format!(
            "the index shares {} case numbers, the CSV {}",
            found.len(),
            expected.len()
        ));
    }
    if options.reference && repeats != reference::CASE_NUMBER_REPEATS {
        details.push(format!(
            "the reference CSV has {} repeats",
            reference::CASE_NUMBER_REPEATS
        ));
    }
    Check::new(
        CRITERION,
        details.is_empty(),
        format!(
            "{} shared case numbers cover {} IDs ({} repeats); in the index each lists every ID \
             that shares it",
            thousands(numbers),
            thousands(ids),
            thousands(repeats)
        ),
    )
    .with_details(details)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn column_positions_name_the_columns_they_stand_for() {
        let columns = [
            (column::DATE, "Date"),
            (column::PRIMARY_TYPE, "Primary Type"),
            (column::WARD, "Ward"),
            (column::COMMUNITY_AREA, "Community Area"),
            (column::X_COORDINATE, "X Coordinate"),
            (column::LATITUDE, "Latitude"),
        ];
        for (position, name) in columns {
            assert_eq!(HEADER[position], name);
        }
    }
}
