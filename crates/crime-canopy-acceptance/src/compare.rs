//! The comparison mode (canopy-db-generador PRD, KG8): the active versions of two output folders,
//! each of either engine, compared table by table and entry by entry.
//!
//! A folder holds a CanopyDB dataset when its manifest has `schema_version` `canopydb-2`, and an
//! SSTable dataset when it has `schema_version` 2 and the original layout, read with the original
//! verifier's LevelDB table reader. When both open, there is one `COMPARE-<table>` row per table;
//! otherwise a single `COMPARE` row, FAIL, with the reason.
//!
//! Both sides of a table are read at once, each by its own thread, and compared as they arrive, so
//! no table is ever held in memory whole.

use std::path::Path;
use std::sync::mpsc::{Receiver, SyncSender, sync_channel};
use std::thread;

use crime_acceptance::dataset::{self, Entry, MANIFEST_FILE, PRIMARY, SstableTables, Tables};
use crime_acceptance::report::thousands;
use crime_acceptance::verify::describe_key;
use crime_acceptance::{Check, Report};
use foundationdb_tuple::{pack, unpack};

use crate::canopy::{BASE_DIR, CanopyTables, TABLES};
use crate::manifest::{self, Engine};

/// Entries sent at a time from a reader to the comparison.
const BATCH_ENTRIES: usize = 4096;
/// Batches a reader can get ahead of the comparison.
const BATCHES_AHEAD: usize = 4;

/// Compares the datasets of the output folders `a` and `b`. `progress` receives a short line
/// before each long step.
pub fn compare(a: &Path, b: &Path, progress: &mut dyn FnMut(&str)) -> Report {
    let mut report = Report::default();
    progress("opening A");
    let side_a = Side::open(a);
    progress("opening B");
    let side_b = Side::open(b);
    match (side_a, side_b) {
        (Ok(side_a), Ok(side_b)) => {
            for table in TABLES {
                progress(&format!("comparing {table}"));
                report
                    .checks
                    .push(compare_table(table, side_a.tables(), side_b.tables()));
            }
        }
        (side_a, side_b) => {
            let reasons: Vec<String> = [("A", a, side_a.err()), ("B", b, side_b.err())]
                .into_iter()
                .filter_map(|(label, folder, error)| {
                    error.map(|error| format!("{label} {}: {error}", folder.display()))
                })
                .collect();
            report.checks.push(
                Check::new("COMPARE", false, reasons[0].clone())
                    .with_details(reasons[1..].to_vec()),
            );
        }
    }
    report
}

/// The tables of one output folder.
enum Side {
    CanopyDb(CanopyTables),
    Sstable(SstableTables),
}

impl Side {
    fn open(output: &Path) -> Result<Self, String> {
        let neither = |error: String| format!("not a dataset of either engine: {error}");
        let version = dataset::resolve_current(output).map_err(neither)?;
        match manifest::identify(&version).map_err(neither)? {
            (Engine::CanopyDb, _) => {
                CanopyTables::open(&version.join(BASE_DIR)).map(Self::CanopyDb)
            }
            (Engine::Sstable, _) => {
                dataset::read_manifest(&version.join(MANIFEST_FILE)).map_err(neither)?;
                let expected = dataset::expected_files();
                let found = dataset::folder_contents(&version).map_err(neither)?;
                if found != expected {
                    return Err(neither(format!(
                        "the SSTable version folder holds {found:?}; expected exactly {expected:?}"
                    )));
                }
                Ok(Self::Sstable(SstableTables::new(&version)))
            }
        }
    }

    fn tables(&self) -> &dyn Tables {
        match self {
            Self::CanopyDb(tables) => tables,
            Self::Sstable(tables) => tables,
        }
    }
}

/// Sends the entries of `table` in batches; stops reading early only if the comparison is gone.
fn stream(tables: &dyn Tables, table: &str, sender: SyncSender<Vec<Entry>>) -> Result<(), String> {
    let mut batch = Vec::with_capacity(BATCH_ENTRIES);
    let mut connected = true;
    tables.scan(table, &mut |key, value| {
        if !connected {
            return;
        }
        batch.push((key.to_vec(), value.to_vec()));
        if batch.len() == BATCH_ENTRIES {
            let full = std::mem::replace(&mut batch, Vec::with_capacity(BATCH_ENTRIES));
            connected = sender.send(full).is_ok();
        }
    })?;
    if connected && !batch.is_empty() {
        let _ = sender.send(batch);
    }
    Ok(())
}

/// What walking both sides of a table found.
struct Walk {
    entries_a: u64,
    entries_b: u64,
    /// Position from 1 and description of the first difference.
    first_difference: Option<(u64, String)>,
}

fn walk(table: &str, a: Receiver<Vec<Entry>>, b: Receiver<Vec<Entry>>) -> Walk {
    let mut a = a.into_iter().flatten();
    let mut b = b.into_iter().flatten();
    let mut walk = Walk {
        entries_a: 0,
        entries_b: 0,
        first_difference: None,
    };
    let mut position = 0;
    loop {
        let (entry_a, entry_b) = (a.next(), b.next());
        if entry_a.is_none() && entry_b.is_none() {
            return walk;
        }
        position += 1;
        walk.entries_a += u64::from(entry_a.is_some());
        walk.entries_b += u64::from(entry_b.is_some());
        if walk.first_difference.is_some() {
            continue;
        }
        let difference = match (&entry_a, &entry_b) {
            (Some((key_a, value_a)), Some((key_b, value_b))) if key_a == key_b => {
                (value_a != value_b).then(|| format!("{} values differ", describe(table, key_a)))
            }
            _ => {
                let side = |entry: &Option<Entry>| {
                    entry
                        .as_ref()
                        .map_or_else(|| "none".to_owned(), |(key, _)| describe(table, key))
                };
                Some(format!("A {}, B {}", side(&entry_a), side(&entry_b)))
            }
        };
        walk.first_difference = difference.map(|text| (position, text));
    }
}

/// A key as its decoded tuple, with the original verifier's formatting: `(id,)` for the primary
/// table, `(value, id)` for an index; its bytes when it does not decode.
fn describe(table: &str, key: &[u8]) -> String {
    if table != PRIMARY {
        return describe_key(key);
    }
    match unpack::<(i64,)>(key) {
        Ok((id,)) if pack(&(id,)) == key => format!("({id},)"),
        _ => format!("{key:02x?}"),
    }
}

fn compare_table(table: &str, a: &dyn Tables, b: &dyn Tables) -> Check {
    let criterion = format!("COMPARE-{table}");
    let (walk, read_a, read_b) = thread::scope(|scope| {
        let (sender_a, receiver_a) = sync_channel(BATCHES_AHEAD);
        let (sender_b, receiver_b) = sync_channel(BATCHES_AHEAD);
        let reader_a = scope.spawn(move || stream(a, table, sender_a));
        let reader_b = scope.spawn(move || stream(b, table, sender_b));
        let walk = walk(table, receiver_a, receiver_b);
        let joined = |reader: thread::ScopedJoinHandle<'_, Result<(), String>>| {
            reader
                .join()
                .unwrap_or_else(|_| Err(format!("the reader of {table} panicked")))
        };
        (walk, joined(reader_a), joined(reader_b))
    });
    let errors: Vec<String> = [("A", read_a), ("B", read_b)]
        .into_iter()
        .filter_map(|(label, read)| read.err().map(|error| format!("{label}: {error}")))
        .collect();
    if !errors.is_empty() {
        return Check::new(criterion, false, errors.join("; "));
    }
    match walk.first_difference {
        None => Check::new(
            criterion,
            true,
            format!(
                "{} entries on each side, equal keys and values",
                thousands(walk.entries_a)
            ),
        ),
        Some((position, difference)) => Check::new(
            criterion,
            false,
            format!(
                "A {} entries, B {} entries; first difference at entry {}: {difference}",
                thousands(walk.entries_a),
                thousands(walk.entries_b),
                thousands(position)
            ),
        ),
    }
}
