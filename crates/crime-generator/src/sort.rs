//! External merge sort with bounded memory.
//!
//! Items are buffered in memory up to the segment size; every full buffer is sorted and written
//! to disk as one segment file, and at the end all segments are merged lazily, one item per
//! segment in memory, while the caller iterates. When everything fits in one buffer, nothing
//! touches the disk.
//!
//! Each sort works in a fresh folder of its own inside the configured temporary folder, so it
//! never touches other files, and that folder is deleted with every segment when the merged
//! stream is closed or dropped, or when an unfinished sort is dropped.
//!
//! The sort is stable: the in-memory sort is stable and the merge breaks ties by segment, which
//! follows input order. With a total order, as the generator uses, the output does not depend on
//! the segment size at all.
//!
//! Every segment stays open during the merge, with a read buffer each, so very small segment
//! sizes on large inputs mean many open files; the default keeps the full reference CSV at a few
//! dozen segments.
//!
//! The `extsort` crate (0.5.0) was evaluated first and rejected: when the input fills its last
//! segment exactly it returns an empty stream, and with an explicit sort folder it writes
//! segments named `0`, `1`, ... into it, truncating existing files, and never deletes them.

use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::fs::{File, OpenOptions};
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::num::NonZeroUsize;
use std::path::Path;

use tempfile::TempDir;

/// Items held in memory before a segment is written to disk.
pub const DEFAULT_SEGMENT_SIZE: NonZeroUsize = NonZeroUsize::new(250_000).unwrap();

/// Name prefix of the folder each sort creates inside the temporary folder.
pub const SORT_DIR_PREFIX: &str = "crime-generator-sort-";

/// Buffer of each segment file while it is written and while it is merged.
const SEGMENT_BUFFER_BYTES: usize = 64 * 1024;

/// Order of a sort.
pub type Compare<T> = fn(&T, &T) -> Ordering;

/// How items are written to and read back from a segment file.
pub trait Sortable: Sized {
    fn encode<W: Write>(&self, writer: &mut W) -> io::Result<()>;

    /// Reads the next item: `Ok(None)` at a clean end of the segment, an error if the segment
    /// ends in the middle of an item.
    fn decode<R: Read>(reader: &mut R) -> io::Result<Option<Self>>;
}

/// A sort that is still receiving items.
pub struct ExternalSort<T: Sortable> {
    compare: Compare<T>,
    segment_size: usize,
    buffer: Vec<T>,
    segment_count: usize,
    dir: TempDir,
}

impl<T: Sortable> ExternalSort<T> {
    /// Starts a sort whose segments go to a new folder inside `temp_dir`. At most `segment_size`
    /// items are held in memory at a time.
    pub fn new(
        temp_dir: &Path,
        segment_size: NonZeroUsize,
        compare: Compare<T>,
    ) -> io::Result<Self> {
        let dir = tempfile::Builder::new()
            .prefix(SORT_DIR_PREFIX)
            .tempdir_in(temp_dir)?;
        Ok(Self {
            compare,
            segment_size: segment_size.get(),
            buffer: Vec::new(),
            segment_count: 0,
            dir,
        })
    }

    pub fn push(&mut self, item: T) -> io::Result<()> {
        self.buffer.push(item);
        if self.buffer.len() >= self.segment_size {
            self.write_segment()?;
        }
        Ok(())
    }

    /// Ends the input and returns the merged stream.
    pub fn finish(mut self) -> io::Result<Sorted<T>> {
        let compare = self.compare;
        if self.segment_count == 0 {
            self.buffer.sort_by(compare);
            return Ok(Sorted {
                source: Source::Memory(std::mem::take(&mut self.buffer).into_iter()),
                segment_count: 0,
                dir: self.dir,
            });
        }
        if !self.buffer.is_empty() {
            self.write_segment()?;
        }
        let mut segments = Vec::with_capacity(self.segment_count);
        let mut heap = BinaryHeap::with_capacity(self.segment_count);
        for index in 0..self.segment_count {
            let file = File::open(segment_path(self.dir.path(), index))?;
            let mut reader = BufReader::with_capacity(SEGMENT_BUFFER_BYTES, file);
            if let Some(item) = T::decode(&mut reader)? {
                heap.push(Head {
                    item,
                    segment: index,
                    compare,
                });
            }
            segments.push(reader);
        }
        Ok(Sorted {
            source: Source::Merge { segments, heap },
            segment_count: self.segment_count,
            dir: self.dir,
        })
    }

    /// Sorts the buffer and writes it as the next segment.
    fn write_segment(&mut self) -> io::Result<()> {
        self.buffer.sort_by(self.compare);
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(segment_path(self.dir.path(), self.segment_count))?;
        let mut writer = BufWriter::with_capacity(SEGMENT_BUFFER_BYTES, file);
        for item in self.buffer.drain(..) {
            item.encode(&mut writer)?;
        }
        writer
            .into_inner()
            .map_err(io::IntoInnerError::into_error)?;
        self.segment_count += 1;
        Ok(())
    }
}

fn segment_path(dir: &Path, index: usize) -> std::path::PathBuf {
    dir.join(format!("segment-{index:06}"))
}

/// Reads the fixed-size start of the next item of a segment into `header`: `false` when the
/// segment was already at its end, `InvalidData` when it ends inside the header.
pub(crate) fn read_item_start(reader: &mut impl Read, header: &mut [u8]) -> io::Result<bool> {
    let mut filled = 0;
    while filled < header.len() {
        match reader.read(&mut header[filled..]) {
            Ok(0) if filled == 0 => return Ok(false),
            Ok(0) => return Err(truncated_item()),
            Ok(read) => filled += read,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => return Err(error),
        }
    }
    Ok(true)
}

/// Reads the rest of an item whose start was read: the end of the segment here is
/// `InvalidData`, because the item is incomplete.
pub(crate) fn read_item_rest(reader: &mut impl Read, rest: &mut [u8]) -> io::Result<()> {
    reader.read_exact(rest).map_err(|error| {
        if error.kind() == io::ErrorKind::UnexpectedEof {
            truncated_item()
        } else {
            error
        }
    })
}

/// A segment that ends in the middle of an item, or holds an item that cannot exist.
pub(crate) fn corrupt_item(reason: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, reason)
}

fn truncated_item() -> io::Error {
    corrupt_item("a sort segment ends in the middle of an entry")
}

/// The merged, sorted stream. Its folder is deleted by [`Sorted::close`] or on drop.
pub struct Sorted<T: Sortable> {
    // Declared before `dir` so it is dropped first: the segment files close before the folder
    // is deleted, which Windows requires.
    source: Source<T>,
    segment_count: usize,
    dir: TempDir,
}

enum Source<T: Sortable> {
    /// Everything fit in one buffer.
    Memory(std::vec::IntoIter<T>),
    /// One reader per segment and a heap with the next item of each segment.
    Merge {
        segments: Vec<BufReader<File>>,
        heap: BinaryHeap<Head<T>>,
    },
}

impl<T: Sortable> Sorted<T> {
    /// Segments written to disk; 0 when everything fit in memory.
    pub fn segment_count(&self) -> usize {
        self.segment_count
    }

    /// Deletes the segments and their folder, reporting a failure that a drop would hide.
    pub fn close(self) -> io::Result<()> {
        let Self { source, dir, .. } = self;
        drop(source);
        dir.close()
    }
}

impl<T: Sortable> Iterator for Sorted<T> {
    type Item = io::Result<T>;

    fn next(&mut self) -> Option<Self::Item> {
        match &mut self.source {
            Source::Memory(items) => items.next().map(Ok),
            Source::Merge { segments, heap } => {
                let Head {
                    item,
                    segment,
                    compare,
                } = heap.pop()?;
                match T::decode(&mut segments[segment]) {
                    Ok(Some(next)) => heap.push(Head {
                        item: next,
                        segment,
                        compare,
                    }),
                    Ok(None) => {}
                    Err(error) => {
                        // The stream cannot be trusted any more: end it after the error.
                        heap.clear();
                        return Some(Err(error));
                    }
                }
                Some(Ok(item))
            }
        }
    }
}

/// The next item of one segment, ordered so that [`BinaryHeap`], a max-heap, pops the smallest
/// item first and, among equal items, the one from the earliest segment.
struct Head<T> {
    item: T,
    segment: usize,
    compare: Compare<T>,
}

impl<T> Ord for Head<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.compare)(&other.item, &self.item).then_with(|| other.segment.cmp(&self.segment))
    }
}

impl<T> PartialOrd for Head<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T> PartialEq for Head<T> {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl<T> Eq for Head<T> {}
