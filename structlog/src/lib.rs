//! # structlog
//!
//! An append-only file of serde-serializable values.
//!
//! * [`LogWriter`] continuously appends items to a file.
//! * [`LogReader`] (or [`iter`]) reads that file back as an [`Iterator`] of items.
//!
//! ## File format
//!
//! ```text
//! header : 8 bytes  b"SLOG\0\0\0\x01"
//! record : u32 LE payload length | u32 LE CRC-32 of payload | payload (postcard)
//! ```
//!
//! Records are self-delimiting and checksummed, so a crash mid-write leaves at
//! worst one torn record at the tail. Readers stop cleanly at a torn tail and
//! [`LogWriter::open`] trims it off before appending more.
//!
//! ```no_run
//! use serde::{Serialize, Deserialize};
//!
//! #[derive(Serialize, Deserialize, Debug)]
//! struct Event { id: u64, name: String }
//!
//! let mut w = structlog::LogWriter::<Event>::open("events.log")?;
//! w.append(&Event { id: 1, name: "hello".into() })?;
//! w.flush()?;
//!
//! for ev in structlog::iter::<Event, _>("events.log")? {
//!     println!("{:?}", ev?);
//! }
//! # Ok::<(), structlog::Error>(())
//! ```

use std::fs::{File, OpenOptions};
use std::io::{self, BufReader, BufWriter, Read, Seek, SeekFrom, Write};
use std::marker::PhantomData;
use std::path::Path;

use serde::{de::DeserializeOwned, Serialize};

const MAGIC: [u8; 8] = *b"SLOG\0\0\0\x01";
const RECORD_HEADER_LEN: u64 = 8;

/// Errors from reading or writing a log.
#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    /// Serialization or deserialization of an item failed.
    Codec(postcard::Error),
    /// The file does not start with the expected header.
    BadMagic,
    /// A complete record failed its checksum (data corruption, not a torn tail).
    Corrupt { offset: u64 },
    /// The item serializes to more than `u32::MAX` bytes.
    TooLarge,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Io(e) => write!(f, "io error: {e}"),
            Error::Codec(e) => write!(f, "codec error: {e}"),
            Error::BadMagic => write!(f, "not a structlog file (bad header)"),
            Error::Corrupt { offset } => write!(f, "corrupt record at byte offset {offset}"),
            Error::TooLarge => write!(f, "item too large to store"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Io(e) => Some(e),
            Error::Codec(e) => Some(e),
            _ => None,
        }
    }
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Error::Io(e)
    }
}
impl From<postcard::Error> for Error {
    fn from(e: postcard::Error) -> Self {
        Error::Codec(e)
    }
}

pub type Result<T> = std::result::Result<T, Error>;

/// Result of trying to read one raw frame.
enum Frame {
    Record(Vec<u8>),
    /// Clean end of file, or an incomplete (torn) trailing record.
    End,
}

/// Reads one frame; `offset` is advanced past it on success.
fn read_frame<R: Read>(r: &mut R, offset: &mut u64) -> Result<Frame> {
    let mut hdr = [0u8; RECORD_HEADER_LEN as usize];
    if !read_full(r, &mut hdr)? {
        return Ok(Frame::End);
    }
    let len = u32::from_le_bytes(hdr[0..4].try_into().unwrap()) as u64;
    let crc = u32::from_le_bytes(hdr[4..8].try_into().unwrap());

    // `take` + `read_to_end` avoids trusting a possibly-garbage length for allocation.
    let mut payload = Vec::new();
    let got = r.by_ref().take(len).read_to_end(&mut payload)? as u64;
    if got < len {
        return Ok(Frame::End); // torn tail
    }
    if crc32fast::hash(&payload) != crc {
        return Err(Error::Corrupt { offset: *offset });
    }
    *offset += RECORD_HEADER_LEN + len;
    Ok(Frame::Record(payload))
}

/// Fills `buf` completely. Returns `false` if EOF was hit before that.
fn read_full<R: Read>(r: &mut R, buf: &mut [u8]) -> io::Result<bool> {
    let mut filled = 0;
    while filled < buf.len() {
        match r.read(&mut buf[filled..]) {
            Ok(0) => return Ok(false),
            Ok(n) => filled += n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e),
        }
    }
    Ok(true)
}

// ---------------------------------------------------------------------------
// Writer
// ---------------------------------------------------------------------------

/// Appends items of type `T` to a log file.
///
/// Writes are buffered; call [`flush`](Self::flush) to push them to the OS and
/// [`sync`](Self::sync) to force them to disk. The buffer is flushed on drop
/// (errors there are ignored, so flush explicitly if you care).
pub struct LogWriter<T> {
    out: BufWriter<File>,
    scratch: Vec<u8>,
    _marker: PhantomData<fn(&T)>,
}

impl<T: Serialize> LogWriter<T> {
    /// Opens `path` for appending, creating it if needed.
    ///
    /// If the file exists, it is scanned (checksums only, no deserialization)
    /// and any torn trailing record from an earlier crash is truncated away.
    /// This is O(file size). A checksum failure in the *middle* of the file
    /// returns [`Error::Corrupt`] rather than silently discarding data.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)?;

        let len = file.metadata()?.len();
        if len == 0 {
            file.write_all(&MAGIC)?;
        } else {
            let valid_end = scan_valid_prefix(&mut file)?;
            if valid_end < len {
                file.set_len(valid_end)?;
            }
        }
        file.seek(SeekFrom::End(0))?;

        Ok(Self {
            out: BufWriter::new(file),
            scratch: Vec::new(),
            _marker: PhantomData,
        })
    }

    /// Appends one item.
    pub fn append(&mut self, item: &T) -> Result<()> {
        self.scratch.clear();
        let payload = postcard::to_extend(item, std::mem::take(&mut self.scratch))?;
        let len = u32::try_from(payload.len()).map_err(|_| Error::TooLarge)?;
        let crc = crc32fast::hash(&payload);

        self.out.write_all(&len.to_le_bytes())?;
        self.out.write_all(&crc.to_le_bytes())?;
        self.out.write_all(&payload)?;
        self.scratch = payload; // reuse the allocation
        Ok(())
    }

    /// Appends every item from an iterator.
    pub fn append_all<'a, I>(&mut self, items: I) -> Result<()>
    where
        I: IntoIterator<Item = &'a T>,
        T: 'a,
    {
        for item in items {
            self.append(item)?;
        }
        Ok(())
    }

    /// Flushes buffered writes to the OS.
    pub fn flush(&mut self) -> Result<()> {
        self.out.flush()?;
        Ok(())
    }

    /// Flushes and `fsync`s, so appended items survive power loss.
    pub fn sync(&mut self) -> Result<()> {
        self.out.flush()?;
        self.out.get_ref().sync_data()?;
        Ok(())
    }
}

/// Returns the byte offset just past the last complete, valid record.
fn scan_valid_prefix(file: &mut File) -> Result<u64> {
    file.seek(SeekFrom::Start(0))?;
    let mut r = BufReader::new(&mut *file);
    check_magic(&mut r)?;
    let mut offset = MAGIC.len() as u64;
    while let Frame::Record(_) = read_frame(&mut r, &mut offset)? {}
    Ok(offset)
}

fn check_magic<R: Read>(r: &mut R) -> Result<()> {
    let mut m = [0u8; 8];
    if !read_full(r, &mut m)? || m != MAGIC {
        return Err(Error::BadMagic);
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Reader
// ---------------------------------------------------------------------------

/// Iterates over the items in a log file.
///
/// Yields `Result<T>`. Iteration ends at EOF or at an incomplete trailing
/// record (which may simply be a write in progress). After the first `Err`,
/// the iterator is finished.
pub struct LogReader<T> {
    inner: Option<BufReader<File>>,
    offset: u64,
    _marker: PhantomData<fn() -> T>,
}

impl<T: DeserializeOwned> LogReader<T> {
    /// Opens `path` for reading. Fails with [`Error::BadMagic`] if it isn't a log file.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let mut r = BufReader::new(File::open(path)?);
        check_magic(&mut r)?;
        Ok(Self {
            inner: Some(r),
            offset: MAGIC.len() as u64,
            _marker: PhantomData,
        })
    }
}

impl<T: DeserializeOwned> Iterator for LogReader<T> {
    type Item = Result<T>;

    fn next(&mut self) -> Option<Self::Item> {
        let r = self.inner.as_mut()?;
        let step = match read_frame(r, &mut self.offset) {
            Ok(Frame::Record(bytes)) => postcard::from_bytes::<T>(&bytes).map_err(Error::from),
            Ok(Frame::End) => {
                self.inner = None;
                return None;
            }
            Err(e) => Err(e),
        };
        if step.is_err() {
            self.inner = None;
        }
        Some(step)
    }
}

/// Convenience: `structlog::iter::<MyStruct, _>(path)`.
pub fn iter<T: DeserializeOwned, P: AsRef<Path>>(path: P) -> Result<LogReader<T>> {
    LogReader::open(path)
}

/// Convenience: read the whole file into a `Vec`.
pub fn read_all<T: DeserializeOwned, P: AsRef<Path>>(path: P) -> Result<Vec<T>> {
    LogReader::open(path)?.collect()
}
