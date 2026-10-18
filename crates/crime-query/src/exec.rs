//! Running a query over an open dataset with the fixed plan of decision C10 (no cost model).
//!
//! The candidates are the IDs that meet every indexed condition:
//!
//! - **With a date condition**, the date index is read over each interval in order (the
//!   intervals are sorted and disjoint, so no ID comes twice) and every other indexed condition
//!   is checked per ID: the ID condition by a binary search in the requested list, each
//!   equality by one point lookup of `(value, id)` per alternative until one is found.
//! - **Without a date condition but with a prefix** ("empieza por", startswith PRD) on an
//!   indexed field, the first such field drives: its index is read over each of its
//!   alternatives in turn, one run of keys per value or prefix, after dropping the alternatives
//!   another prefix of the field already covers, so the runs are disjoint and no ID comes twice.
//!   The other indexed conditions are checked per ID as with a date condition.
//! - **Otherwise**, every alternative of an equality is an ascending stream of IDs; the
//!   alternatives of one condition are joined by a k-way union and the conditions are
//!   intersected by a sorted merge that advances the smaller heads. The ID condition is the
//!   ascending list of the requested IDs that exist.
//!
//! An indexed condition with prefixes that does not drive cannot be checked by a point lookup
//! (the index is keyed by value), so it is checked on the candidate's record, like the filters
//! on fields without an index. The record is read only when one of those checks or the output
//! needs it: a count without them reads no record.
//!
//! Memory grows with the number of conditions and alternatives, never with the result nor with
//! the distinct values a prefix covers (decisions C7 and startswith S7): every stream holds one
//! table iterator, one run is open at a time and nothing collects IDs or records. The order of
//! the results is unspecified (decision C11).

use std::fmt;
use std::time::{Duration, Instant};

use crime_dataset::{CrimeRecord, IndexKind};

use crate::query::{DateInterval, Equality};
use crate::{Dataset, Field, IdStream, IndexValue, Query, ReadError, messages};

/// Why an execution stopped before it completed. [`fmt::Display`] gives the message for the
/// analyst.
#[derive(Debug, Clone)]
pub enum ExecError {
    /// A read of the dataset failed: the result would be partial (exit code 1).
    Read(ReadError),
    /// The cancellation check answered `true` (exit code 130).
    Cancelled,
}

impl fmt::Display for ExecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExecError::Read(error) => error.fmt(f),
            ExecError::Cancelled => f.write_str(&messages::query_cancelled()),
        }
    }
}

impl std::error::Error for ExecError {}

impl From<ReadError> for ExecError {
    fn from(error: ReadError) -> Self {
        ExecError::Read(error)
    }
}

/// What a query reports on stderr when it completes (decision C5, flow step 7).
#[derive(Debug, Clone, PartialEq)]
pub struct Summary {
    /// The results returned, capped by the limit.
    pub count: u64,
    /// From the start of the execution to its end.
    pub elapsed: Duration,
    /// The fields of the indexed conditions, in CSV order.
    pub indexes: Vec<Field>,
    /// The fields filtered without an index, in CSV order.
    pub filters: Vec<Field>,
}

impl Summary {
    /// `<n> resultados en <t> s. Índices usados: <campos>. Filtros sin índice: <campos>.`
    pub fn stats_line(&self) -> String {
        let names = |fields: &[Field]| fields.iter().map(|field| field.name()).collect::<Vec<_>>();
        messages::stats(
            self.count,
            self.elapsed,
            &names(&self.indexes),
            &names(&self.filters),
        )
    }
}

/// One running query.
///
/// Iterating it yields the records of the result one at a time, read lazily; a failure is the
/// last item, and after it, after the last record, or after the limit is reached, it yields
/// `None`. [`Execution::count`] counts the results instead, without reading records unless a
/// filter needs them. [`Execution::summary`] reports the run once it has ended.
pub struct Execution<'a> {
    dataset: &'a Dataset,
    query: &'a Query,
    cancel: &'a dyn Fn() -> bool,
    candidates: Candidates<'a>,
    /// The indexed conditions with prefixes that do not drive, checked on each record.
    on_record: Vec<&'a Equality>,
    warnings: Vec<String>,
    started: Instant,
    count: u64,
    /// Set when the execution ends, and from then on reported.
    elapsed: Option<Duration>,
}

impl<'a> Execution<'a> {
    /// Starts `query` on `dataset`: checks that every requested ID and case number exists,
    /// keeping a warning for each one that does not, and prepares the plan. `cancel` is polled
    /// before each of those checks and then for every ID examined; once it answers `true` the
    /// execution ends with [`ExecError::Cancelled`], here or from the iterator.
    pub fn start(
        dataset: &'a Dataset,
        query: &'a Query,
        cancel: &'a dyn Fn() -> bool,
    ) -> Result<Self, ExecError> {
        let started = Instant::now();
        let mut warnings = Vec::new();
        let mut existing = Vec::new();
        for &id in query.ids().unwrap_or_default() {
            if cancel() {
                return Err(ExecError::Cancelled);
            }
            if dataset.id_exists(id)? {
                existing.push(id);
            } else {
                warnings.push(messages::missing_id(id));
            }
        }
        let case_numbers = query
            .equalities()
            .iter()
            .filter(|equality| equality.index() == IndexKind::CaseNumber);
        for equality in case_numbers {
            for (value, typed) in equality.values().iter().zip(equality.typed()) {
                if cancel() {
                    return Err(ExecError::Cancelled);
                }
                if !dataset.has_value(IndexKind::CaseNumber, value)? {
                    warnings.push(messages::missing_case_number(typed));
                }
            }
        }
        let (by_lookup, with_prefixes): (Vec<&Equality>, Vec<&Equality>) = query
            .equalities()
            .iter()
            .partition(|equality| equality.prefixes().is_empty());
        let mut on_record = with_prefixes;
        let scan = |runs| {
            Candidates::Scan(Box::new(Scan {
                dataset,
                runs,
                stream: None,
                ids: query.ids(),
                equalities: by_lookup.clone(),
            }))
        };
        let candidates = match query.dates() {
            Some(intervals) => scan(Runs::Dates(intervals.iter())),
            None if !on_record.is_empty() => {
                let driver = on_record.remove(0);
                scan(Runs::Field(
                    driver.index(),
                    disjoint_runs(driver).into_iter(),
                ))
            }
            None => {
                let mut sources = Vec::new();
                if query.ids().is_some() {
                    sources.push(Sorted::List(existing.into_iter()));
                }
                for equality in &by_lookup {
                    let mut streams: Vec<IdStream> = equality
                        .values()
                        .iter()
                        .map(|value| dataset.ids_with_value(equality.index(), value))
                        .collect();
                    sources.push(match streams.len() {
                        1 => Sorted::Index(Box::new(streams.pop().expect("one stream"))),
                        _ => Sorted::Union(Union::new(streams)),
                    });
                }
                Candidates::Merge(Intersection::new(sources))
            }
        };
        Ok(Self {
            dataset,
            query,
            cancel,
            candidates,
            on_record,
            warnings,
            started,
            count: 0,
            elapsed: None,
        })
    }

    /// One message per requested ID and per requested case number with no record, in the
    /// order of the query (IDs ascending). They are warnings, not errors.
    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }

    /// Counts the results, reading records only when a check on the record needs them.
    pub fn count(mut self) -> Result<Summary, ExecError> {
        while let Some(item) = self.next_match(false) {
            item?;
        }
        Ok(self.summary())
    }

    /// The run so far; final once the execution has ended.
    pub fn summary(&self) -> Summary {
        let mut indexes = Vec::new();
        if self.query.ids().is_some() {
            indexes.push(Field::Id);
        }
        indexes.extend(self.query.equalities().iter().map(Equality::field));
        if self.query.dates().is_some() {
            indexes.push(Field::Date);
        }
        let mut filters: Vec<Field> = self.query.filters().iter().map(|f| f.field()).collect();
        indexes.sort_by_key(|&field| field.csv_position());
        filters.sort_by_key(|&field| field.csv_position());
        Summary {
            count: self.count,
            elapsed: self.elapsed.unwrap_or_else(|| self.started.elapsed()),
            indexes,
            filters,
        }
    }

    /// The next result, with its record when `want_record` or a check on the record needs it.
    fn next_match(&mut self, want_record: bool) -> Option<Result<Option<CrimeRecord>, ExecError>> {
        if self.elapsed.is_some() {
            return None;
        }
        let checks_record = !self.query.filters().is_empty() || !self.on_record.is_empty();
        let item = self.find(want_record || checks_record);
        // Ended by the limit, by the end of the candidates or by a failure.
        let ended = match &item {
            Some(Ok(_)) => {
                self.count += 1;
                self.query
                    .limit()
                    .is_some_and(|limit| self.count >= limit.get())
            }
            _ => true,
        };
        if ended {
            self.elapsed = Some(self.started.elapsed());
        }
        item
    }

    fn find(&mut self, read_record: bool) -> Option<Result<Option<CrimeRecord>, ExecError>> {
        loop {
            if (self.cancel)() {
                return Some(Err(ExecError::Cancelled));
            }
            let id = match self.candidates.next(self.cancel)? {
                Ok(id) => id,
                Err(error) => return Some(Err(error)),
            };
            if !read_record {
                return Some(Ok(None));
            }
            let record = match self.dataset.record(id) {
                Ok(Some(record)) => record,
                Ok(None) => {
                    let cause = format!("record {id} is in an index but not in the primary table");
                    let error = ReadError::new(self.dataset.folder().to_path_buf(), cause);
                    return Some(Err(error.into()));
                }
                Err(error) => return Some(Err(error.into())),
            };
            let on_record = self
                .on_record
                .iter()
                .all(|equality| equality.matches(&record));
            if on_record && self.query.matches(&record) {
                return Some(Ok(Some(record)));
            }
        }
    }
}

impl Iterator for Execution<'_> {
    type Item = Result<CrimeRecord, ExecError>;

    fn next(&mut self) -> Option<Self::Item> {
        self.next_match(true)
            .map(|item| item.map(|record| record.expect("the record was asked for")))
    }
}

/// The IDs that meet every indexed condition, each once.
enum Candidates<'a> {
    Scan(Box<Scan<'a>>),
    Merge(Intersection),
}

impl Candidates<'_> {
    fn next(&mut self, cancel: &dyn Fn() -> bool) -> Option<Result<i64, ExecError>> {
        match self {
            Candidates::Scan(scan) => scan.next(cancel),
            Candidates::Merge(intersection) => intersection.next(cancel),
        }
    }
}

/// One alternative of an indexed field, read as one run of index keys.
enum Run<'a> {
    Value(&'a IndexValue),
    Prefix(&'a str),
}

/// The disjoint runs that cover every alternative of `equality`: each prefix that no shorter
/// prefix of the field covers, then each value that no prefix covers. "Sin valor" is never
/// covered: it starts with no prefix.
fn disjoint_runs(equality: &Equality) -> Vec<Run<'_>> {
    let prefixes = equality.prefixes();
    let covered = |text: &str, own: Option<&str>| {
        prefixes
            .iter()
            .any(|prefix| Some(prefix.as_str()) != own && text.starts_with(prefix.as_str()))
    };
    let kept = prefixes
        .iter()
        .filter(|prefix| !covered(prefix, Some(prefix)))
        .map(|prefix| Run::Prefix(prefix));
    let values = equality
        .values()
        .iter()
        .filter(|value| !value.as_text().is_some_and(|text| covered(text, None)))
        .map(Run::Value);
    kept.chain(values).collect()
}

/// The runs of keys a [`Scan`] reads, in turn.
enum Runs<'a> {
    /// The date index over each interval.
    Dates(std::slice::Iter<'a, DateInterval>),
    /// The index of one field over each of its disjoint alternatives.
    Field(IndexKind, std::vec::IntoIter<Run<'a>>),
}

impl Runs<'_> {
    /// The stream of the next run, `None` after the last one.
    fn next_stream(&mut self, dataset: &Dataset) -> Option<IdStream> {
        match self {
            Runs::Dates(intervals) => {
                let interval = intervals.next()?;
                Some(dataset.ids_in_date_range(interval.from, interval.to))
            }
            Runs::Field(index, runs) => Some(match runs.next()? {
                Run::Value(value) => dataset.ids_with_value(*index, value),
                Run::Prefix(prefix) => dataset.ids_with_prefix(*index, prefix),
            }),
        }
    }
}

/// An index read over disjoint runs of keys, with the other indexed conditions checked per ID.
struct Scan<'a> {
    dataset: &'a Dataset,
    runs: Runs<'a>,
    stream: Option<IdStream>,
    /// The requested IDs, sorted, when there is an ID condition.
    ids: Option<&'a [i64]>,
    /// The equalities without prefixes, checked by point lookups.
    equalities: Vec<&'a Equality>,
}

impl Scan<'_> {
    fn next(&mut self, cancel: &dyn Fn() -> bool) -> Option<Result<i64, ExecError>> {
        loop {
            if cancel() {
                return Some(Err(ExecError::Cancelled));
            }
            let accepted = self
                .next_id()?
                .and_then(|id| Ok(self.accepts(id)?.then_some(id)));
            match accepted {
                Ok(Some(id)) => return Some(Ok(id)),
                Ok(None) => {}
                Err(error) => return Some(Err(error.into())),
            }
        }
    }

    /// The next ID of the current run, opening the next run when one ends.
    fn next_id(&mut self) -> Option<Result<i64, ReadError>> {
        loop {
            if let Some(item) = self.stream.as_mut().and_then(Iterator::next) {
                return Some(item);
            }
            self.stream = Some(self.runs.next_stream(self.dataset)?);
        }
    }

    fn accepts(&self, id: i64) -> Result<bool, ReadError> {
        if self.ids.is_some_and(|ids| ids.binary_search(&id).is_err()) {
            return Ok(false);
        }
        for equality in &self.equalities {
            let mut found = false;
            for value in equality.values() {
                if self.dataset.has_entry(equality.index(), value, id)? {
                    found = true;
                    break;
                }
            }
            if !found {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

/// An ascending stream of distinct IDs.
enum Sorted {
    List(std::vec::IntoIter<i64>),
    Index(Box<IdStream>),
    Union(Union),
}

impl Sorted {
    fn next(&mut self) -> Option<Result<i64, ReadError>> {
        match self {
            Sorted::List(ids) => ids.next().map(Ok),
            Sorted::Index(stream) => stream.next(),
            Sorted::Union(union) => union.next(),
        }
    }
}

/// The union of ascending streams, ascending and without duplicates.
struct Union {
    streams: Vec<IdStream>,
    /// The current ID of each stream, `None` once it has ended; empty until the first read.
    heads: Vec<Option<i64>>,
}

impl Union {
    fn new(streams: Vec<IdStream>) -> Self {
        Self {
            streams,
            heads: Vec::new(),
        }
    }

    fn next(&mut self) -> Option<Result<i64, ReadError>> {
        if self.heads.is_empty() {
            for stream in &mut self.streams {
                match stream.next().transpose() {
                    Ok(head) => self.heads.push(head),
                    Err(error) => return Some(Err(error)),
                }
            }
        }
        let smallest = self.heads.iter().flatten().min().copied()?;
        for (head, stream) in self.heads.iter_mut().zip(&mut self.streams) {
            if *head == Some(smallest) {
                match stream.next().transpose() {
                    Ok(next) => *head = next,
                    Err(error) => return Some(Err(error)),
                }
            }
        }
        Some(Ok(smallest))
    }
}

/// The intersection of ascending streams by a sorted merge.
struct Intersection {
    sources: Vec<Sorted>,
    /// The current ID of each source; empty until the first read.
    heads: Vec<i64>,
    finished: bool,
}

impl Intersection {
    fn new(sources: Vec<Sorted>) -> Self {
        Self {
            sources,
            heads: Vec::new(),
            finished: false,
        }
    }

    fn next(&mut self, cancel: &dyn Fn() -> bool) -> Option<Result<i64, ExecError>> {
        if self.finished {
            return None;
        }
        let item = self.advance(cancel).transpose();
        if !matches!(item, Some(Ok(_))) {
            self.finished = true;
        }
        item
    }

    /// The next ID every source holds; `None` once any source ends.
    fn advance(&mut self, cancel: &dyn Fn() -> bool) -> Result<Option<i64>, ExecError> {
        if self.heads.is_empty() {
            for source in &mut self.sources {
                match source.next().transpose()? {
                    Some(id) => self.heads.push(id),
                    None => return Ok(None),
                }
            }
        } else {
            // Every head is the ID returned last.
            for (head, source) in self.heads.iter_mut().zip(&mut self.sources) {
                match source.next().transpose()? {
                    Some(id) => *head = id,
                    None => return Ok(None),
                }
            }
        }
        loop {
            let Some(&target) = self.heads.iter().max() else {
                return Ok(None);
            };
            let mut all_equal = true;
            for (head, source) in self.heads.iter_mut().zip(&mut self.sources) {
                while *head < target {
                    if cancel() {
                        return Err(ExecError::Cancelled);
                    }
                    match source.next().transpose()? {
                        Some(id) => *head = id,
                        None => return Ok(None),
                    }
                }
                all_equal &= *head == target;
            }
            if all_equal {
                return Ok(Some(target));
            }
        }
    }
}
