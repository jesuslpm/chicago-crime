//! Minimal reproductions of two CanopyDB 0.2.5 findings of the module's findings register
//! (`docs/canopy-db/hallazgos.md`). Each one uses only CanopyDB and the standard library.
//!
//! - `cargo run -- h2`: a second process cannot open a base that another process has open; it
//!   waits for the file lock and fails after about 5 seconds.
//! - `cargo run -- h3`: there is no read-only opening. A base in a read-only folder cannot be
//!   opened to read it, and opening a base that was closed without a checkpoint, only to read it,
//!   changes its files.
//! - `cargo run --release -- h5`: there is no bulk load. Already sorted entries, as many as the
//!   Chicago crime dataset holds (8,656,904 records and five indexes), can only be written one by
//!   one with `Tree::insert` inside transactions; it prints the time and the size of the base.
//!
//! Every run works in a new folder under the system temporary folder and prints what it observes;
//! it exits with 0 when the behavior of the finding is reproduced and with 1 when it is not.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::time::{Duration, Instant};

use canopydb::{DbOptions, EnvOptions, Environment};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let reproduced = match args.get(1).map(String::as_str) {
        Some("h2") => h2(),
        Some("h2-second-process") => return h2_second_process(Path::new(&args[2])),
        Some("h3") => h3(),
        Some("h5") => h5(),
        _ => {
            eprintln!("usage: canopydb-repro h2 | h3 | h5");
            return ExitCode::from(2);
        }
    };
    if reproduced {
        println!("REPRODUCED");
        ExitCode::SUCCESS
    } else {
        println!("NOT REPRODUCED");
        ExitCode::FAILURE
    }
}

/// A new empty folder under the system temporary folder.
fn new_folder(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("canopydb-repro-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).expect("the folder can be created");
    path
}

/// Creates a base in `path` with one database `db` holding `entries` entries in tree `t`.
fn create_base(path: &Path, entries: u32, use_wal: bool, checkpoint: bool) {
    let environment = Environment::new(path).expect("the base can be created");
    let mut options = DbOptions::default();
    options.use_wal = use_wal;
    let database = environment
        .get_or_create_database_with("db", options)
        .expect("the database can be created");
    let transaction = database.begin_write().expect("a write transaction");
    {
        let mut tree = transaction.get_or_create_tree(b"t").expect("the tree");
        for i in 0..entries {
            tree.insert(&i.to_be_bytes(), b"value").expect("an insert");
        }
    }
    transaction.commit().expect("the commit");
    if checkpoint {
        database.checkpoint().expect("the checkpoint");
    }
}

/// H2: process A keeps the base open; process B tries to open it.
fn h2() -> bool {
    let path = new_folder("h2");
    create_base(&path, 10, false, true);
    let _held = Environment::new(&path).expect("process A opens the base");
    println!("process A ({}) has the base open", std::process::id());
    let started = Instant::now();
    let status = Command::new(std::env::current_exe().expect("the program path"))
        .arg("h2-second-process")
        .arg(&path)
        .status()
        .expect("process B can be started");
    let elapsed = started.elapsed();
    println!(
        "process B ended with code {:?} after {:.1} s",
        status.code(),
        elapsed.as_secs_f64()
    );
    status.code() == Some(3) && elapsed >= Duration::from_secs(4)
}

/// Process B of H2: exits with 3 when the opening fails.
fn h2_second_process(path: &Path) -> ExitCode {
    let started = Instant::now();
    match Environment::new(path) {
        Ok(_) => {
            println!("process B opened the base");
            ExitCode::SUCCESS
        }
        Err(error) => {
            println!(
                "process B could not open the base after {:.1} s: {error} ({error:?})",
                started.elapsed().as_secs_f64()
            );
            ExitCode::from(3)
        }
    }
}

/// H5: the only way to write is `Tree::insert`, even for entries that arrive already sorted.
/// Writes the volume of the Chicago crime dataset: a primary tree of 8,656,904 entries of 138
/// bytes and five index trees of as many entries with empty values, 100,000 entries per
/// transaction, without write-ahead log and with a 64 MiB cache (the generator's settings), then
/// prints the time and the size of the base. It reproduces as long as the load completes: the
/// finding is the missing API, the figures are its cost.
fn h5() -> bool {
    const RECORDS: u64 = 8_656_904;
    let path = new_folder("h5");
    let mut options = EnvOptions::new(&path);
    options.page_cache_size = 64 << 20;
    let environment = Environment::with_options(options).expect("the base can be created");
    let mut db_options = DbOptions::default();
    db_options.use_wal = false;
    let database = environment
        .get_or_create_database_with("db", db_options)
        .expect("the database can be created");
    let value = vec![0x42u8; 138];
    let started = Instant::now();
    for (tree_index, name) in [
        "primary",
        "date",
        "primary_type",
        "district",
        "community_area",
        "case_number",
    ]
    .iter()
    .enumerate()
    {
        let mut next = 0u64;
        while next < RECORDS {
            let transaction = database.begin_write().expect("a write transaction");
            {
                let mut tree = transaction
                    .get_or_create_tree(name.as_bytes())
                    .expect("the tree");
                let end = (next + 100_000).min(RECORDS);
                while next < end {
                    if tree_index == 0 {
                        tree.insert(&next.to_be_bytes(), &value).expect("an insert");
                    } else {
                        let mut key = (next / 1_000).to_be_bytes().to_vec();
                        key.extend_from_slice(b"value-x");
                        key.extend_from_slice(&next.to_be_bytes());
                        tree.insert(&key, &[]).expect("an insert");
                    }
                    next += 1;
                }
            }
            transaction.commit().expect("the commit");
        }
    }
    database.checkpoint().expect("the checkpoint");
    let elapsed = started.elapsed();
    drop(database);
    drop(environment);
    let size: usize = files(&path).values().map(Vec::len).sum();
    println!(
        "h5) {} sorted entries in 6 trees written with Tree::insert in {:.1} s; base size {} bytes",
        RECORDS * 6,
        elapsed.as_secs_f64(),
        size
    );
    let _ = fs::remove_dir_all(&path);
    true
}

/// Name, size and content of every file under `dir`.
fn files(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut all = BTreeMap::new();
    let mut pending = vec![dir.to_path_buf()];
    while let Some(folder) = pending.pop() {
        for entry in fs::read_dir(&folder).expect("the folder can be listed") {
            let path = entry.expect("an entry").path();
            if path.is_dir() {
                pending.push(path);
            } else {
                let name = path.strip_prefix(dir).expect("under").display().to_string();
                all.insert(name, fs::read(&path).expect("the file can be read"));
            }
        }
    }
    all
}

fn set_read_only(dir: &Path, read_only: bool) {
    let mut pending = vec![dir.to_path_buf()];
    while let Some(path) = pending.pop() {
        if path.is_dir() {
            for entry in fs::read_dir(&path).expect("the folder can be listed") {
                pending.push(entry.expect("an entry").path());
            }
        }
        let mut permissions = fs::metadata(&path).expect("metadata").permissions();
        #[allow(clippy::permissions_set_readonly_false)]
        permissions.set_readonly(read_only);
        fs::set_permissions(&path, permissions).expect("the permissions can be set");
    }
}

/// H3: no read-only opening.
fn h3() -> bool {
    // a) A base in a read-only folder cannot be opened to read it.
    let read_only = new_folder("h3-read-only");
    create_base(&read_only, 10, false, true);
    set_read_only(&read_only, true);
    let opened = Environment::with_options(EnvOptions::new(&read_only)).and_then(|environment| {
        let database = environment.get_database("db")?;
        let entries = match database {
            Some(database) => database
                .begin_read()?
                .get_tree(b"t")?
                .map_or(0, |tree| tree.len()),
            None => 0,
        };
        Ok(entries)
    });
    set_read_only(&read_only, false);
    let read_only_fails = match &opened {
        Ok(entries) => {
            println!("a) the read-only base opened and holds {entries} entries");
            false
        }
        Err(error) => {
            println!("a) the read-only base cannot be opened: {error} ({error:?})");
            true
        }
    };

    // b) Opening only to read a base closed without a checkpoint changes its files.
    let unflushed = new_folder("h3-unflushed");
    create_base(&unflushed, 1_000, true, false);
    let before = files(&unflushed);
    {
        let environment = Environment::new(&unflushed).expect("the base opens");
        let database = environment
            .get_database("db")
            .expect("the database opens")
            .expect("the database exists");
        let transaction = database.begin_read().expect("a read transaction");
        let tree = transaction
            .get_tree(b"t")
            .expect("the tree")
            .expect("exists");
        println!("b) read {} entries without writing", tree.len());
    }
    let after = files(&unflushed);
    let mut changed = Vec::new();
    for name in before.keys().chain(after.keys()) {
        let (old, new) = (before.get(name), after.get(name));
        if old != new && !changed.contains(name) {
            changed.push(name.clone());
            println!(
                "b) {name}: {} -> {}",
                old.map_or("missing".to_owned(), |bytes| format!(
                    "{} bytes",
                    bytes.len()
                )),
                new.map_or("missing".to_owned(), |bytes| format!(
                    "{} bytes",
                    bytes.len()
                ))
            );
        }
    }
    if changed.is_empty() {
        println!("b) reading changed no file");
    }
    read_only_fails && !changed.is_empty()
}
