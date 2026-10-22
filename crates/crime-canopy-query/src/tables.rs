//! The six tables of an open CanopyDB base, read through one read transaction.
//!
//! CanopyDB 0.2.5 lets a transaction hold each tree open only once at a time (a second
//! `get_tree` of an open tree fails with `TreeAlreadyOpen`), while the query engine reads several
//! streams of one index at once (a union of values) and looks up entries between them. So no tree
//! is ever kept open across calls: a lookup opens its tree, reads one entry and closes it, and a
//! [`PagedCursor`] reads a page of keys and closes the tree again, resuming after the last key it
//! read. One read transaction serves the whole run, so every read sees the same snapshot.
//!
//! Every call into CanopyDB goes through [`guarded`]: an error or a panic of the engine becomes
//! a [`ReadError`] with the engine's text as its detail line (decision KC5), never an abnormal
//! exit.

use std::cell::Cell;
use std::ops::Bound;
use std::panic::{self, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Once;

use canopydb::{Database, Environment, ReadTransaction, Tree};
use crime_dataset::IndexKind;
use crime_dataset::canopy::{PRIMARY_TABLE_NAME, engine_detail, panic_detail};
use crime_query::{KeyCursor, ReadError, Tables};

/// Keys a cursor reads at a time: its memory is bounded by a page, whatever the result (C7).
const PAGE_KEYS: usize = 1024;

/// How a call into the engine failed.
pub(crate) enum EngineFailure {
    Error(canopydb::Error),
    /// The detail line of a panic.
    Panic(String),
}

impl EngineFailure {
    /// The detail line the analyst sees (decision KC5).
    pub(crate) fn detail(&self) -> String {
        match self {
            EngineFailure::Error(error) => engine_detail(&error.to_string()),
            EngineFailure::Panic(detail) => detail.clone(),
        }
    }
}

thread_local! {
    /// Whether this thread is inside a call into the engine, where a panic is reported as an
    /// engine failure instead of by the panic hook.
    static IN_ENGINE: Cell<bool> = const { Cell::new(false) };
}

/// Runs `operation`, a call into CanopyDB, turning a panic into an [`EngineFailure`]. The panic
/// message is not printed: the detail line carries it. A panic anywhere else still goes to the
/// previous hook.
pub(crate) fn guarded<T>(
    operation: impl FnOnce() -> Result<T, canopydb::Error>,
) -> Result<T, EngineFailure> {
    static QUIET_HOOK: Once = Once::new();
    QUIET_HOOK.call_once(|| {
        let previous = panic::take_hook();
        panic::set_hook(Box::new(move |info| {
            if !IN_ENGINE.with(Cell::get) {
                previous(info);
            }
        }));
    });
    let outer = IN_ENGINE.with(|flag| flag.replace(true));
    let result = panic::catch_unwind(AssertUnwindSafe(operation));
    IN_ENGINE.with(|flag| flag.set(outer));
    match result {
        Ok(result) => result.map_err(EngineFailure::Error),
        Err(payload) => Err(EngineFailure::Panic(panic_detail(payload.as_ref()))),
    }
}

/// The open base: one read transaction over its `crime` database.
pub struct CanopyTables {
    // Dropped in this order: the transaction before the database, the database before the
    // environment that holds it.
    transaction: Rc<ReadTransaction>,
    _database: Database,
    _environment: Environment,
    /// The output folder exactly as the analyst gave it, for the messages.
    folder: PathBuf,
}

impl CanopyTables {
    pub(crate) fn new(
        transaction: ReadTransaction,
        database: Database,
        environment: Environment,
        folder: PathBuf,
    ) -> Self {
        Self {
            transaction: Rc::new(transaction),
            _database: database,
            _environment: environment,
            folder,
        }
    }

    /// Runs `read` on the tree `name`, open only for the call.
    fn with_tree<T>(
        &self,
        name: &str,
        read: impl FnOnce(&Tree<'_>) -> Result<T, canopydb::Error>,
    ) -> Result<T, ReadError> {
        read_tree(&self.transaction, name, &self.folder, read)
    }
}

/// Runs `read` on the tree `name` of `transaction`, open only for the call. A tree that is not
/// there, an error or a panic of the engine is a [`ReadError`].
fn read_tree<T>(
    transaction: &ReadTransaction,
    name: &str,
    folder: &Path,
    read: impl FnOnce(&Tree<'_>) -> Result<T, canopydb::Error>,
) -> Result<T, ReadError> {
    let found = guarded(|| {
        let Some(tree) = transaction.get_tree(name.as_bytes())? else {
            return Ok(None);
        };
        read(&tree).map(Some)
    });
    match found {
        Ok(Some(value)) => Ok(value),
        Ok(None) => Err(ReadError::new(
            folder.to_path_buf(),
            format!("the CanopyDB base has no tree {name:?}"),
        )),
        Err(failure) => {
            let detail = failure.detail();
            Err(ReadError::with_detail(
                folder.to_path_buf(),
                detail.clone(),
                detail,
            ))
        }
    }
}

impl Tables for CanopyTables {
    fn primary_value(&self, key: &[u8]) -> Result<Option<Vec<u8>>, ReadError> {
        self.with_tree(PRIMARY_TABLE_NAME, |tree| {
            Ok(tree.get(key)?.map(|value| value.to_vec()))
        })
    }

    fn index_contains(&self, kind: IndexKind, key: &[u8]) -> Result<bool, ReadError> {
        self.with_tree(kind.name(), |tree| Ok(tree.get(key)?.is_some()))
    }

    fn index_cursor(&self, kind: IndexKind) -> Box<dyn KeyCursor> {
        Box::new(PagedCursor {
            transaction: Rc::clone(&self.transaction),
            tree: kind.name(),
            folder: self.folder.clone(),
            keys: Vec::new(),
            position: 0,
            more: false,
        })
    }
}

/// A [`KeyCursor`] that reads [`PAGE_KEYS`] keys of a tree at a time and resumes after the last
/// one, with the tree open only while it reads a page.
struct PagedCursor {
    transaction: Rc<ReadTransaction>,
    tree: &'static str,
    folder: PathBuf,
    /// The page being read.
    keys: Vec<Vec<u8>>,
    /// The current key in `keys`.
    position: usize,
    /// Whether the tree may hold keys after the page.
    more: bool,
}

impl PagedCursor {
    /// Reads the page of keys from `from` on; `Ok(false)` when it is empty.
    fn fill(&mut self, from: Bound<Vec<u8>>) -> Result<bool, ReadError> {
        let keys = read_tree(&self.transaction, self.tree, &self.folder, |tree| {
            let bounds: (Bound<&[u8]>, Bound<&[u8]>) = (
                match &from {
                    Bound::Included(key) => Bound::Included(key.as_slice()),
                    Bound::Excluded(key) => Bound::Excluded(key.as_slice()),
                    Bound::Unbounded => Bound::Unbounded,
                },
                Bound::Unbounded,
            );
            let mut keys = Vec::with_capacity(PAGE_KEYS);
            for key in tree.range_keys::<&[u8]>(bounds)?.take(PAGE_KEYS) {
                keys.push(key?.to_vec());
            }
            Ok(keys)
        })?;
        self.more = keys.len() == PAGE_KEYS;
        self.keys = keys;
        self.position = 0;
        Ok(!self.keys.is_empty())
    }
}

impl KeyCursor for PagedCursor {
    fn seek(&mut self, start: &[u8]) -> Result<bool, ReadError> {
        self.fill(Bound::Included(start.to_vec()))
    }

    fn advance(&mut self) -> Result<bool, ReadError> {
        if self.position + 1 < self.keys.len() {
            self.position += 1;
            return Ok(true);
        }
        if !self.more {
            self.keys.clear();
            self.position = 0;
            return Ok(false);
        }
        let last = self.keys.pop().expect("a full page has a last key");
        self.fill(Bound::Excluded(last))
    }

    fn key(&self) -> &[u8] {
        &self.keys[self.position]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_panic_of_the_engine_becomes_its_detail_line() {
        let failure = guarded::<()>(|| panic!("page 7 is\nnot a page")).expect_err("a failure");
        assert_eq!(
            failure.detail(),
            "Detalle del error de CanopyDB: page 7 is not a page"
        );
        // The flag is down again: a later panic outside the engine reaches the previous hook.
        assert!(!IN_ENGINE.with(Cell::get));
        assert_eq!(guarded(|| Ok(7)).ok(), Some(7));
    }

    #[test]
    fn an_error_of_the_engine_becomes_its_detail_line() {
        let failure =
            guarded::<()>(|| Err(canopydb::Error::EnvironmentLocked)).expect_err("a failure");
        assert!(matches!(
            failure,
            EngineFailure::Error(canopydb::Error::EnvironmentLocked)
        ));
        assert!(
            failure
                .detail()
                .starts_with("Detalle del error de CanopyDB: ")
        );
    }
}
