//! Reading a ZIP archive, which is what a 3MF file is (an OPC package).
//!
//! Enough of the format to find a named part and hand back its bytes: the
//! central directory, stored and deflated entries, and nothing else. Written
//! here for the reason the export crate's writer is -- one self-contained
//! binary, no dependency tree -- and paired with [`crate::inflate`], since a
//! 3MF written by any other program is deflated.
//!
//! Only the central directory is trusted for an entry's sizes. A local header
//! is allowed to say nothing at all (the streaming case, where the sizes follow
//! the data), and a reader that believes it instead reads zeroes.

use crate::inflate::inflate;

/// What a ZIP entry says about itself, before its data is read.
struct Entry {
    name: String,
    stored: bool,
    compressed_size: usize,
    uncompressed_size: usize,
    /// Where the entry's *local* header starts.
    offset: usize,
}

/// The parts of `data`, in the order the central directory lists them.
pub struct Archive<'a> {
    data: &'a [u8],
    entries: Vec<Entry>,
}

impl<'a> Archive<'a> {
    /// Read the central directory. Nothing is decompressed yet: a 3MF holds
    /// parts an importer never looks at -- thumbnails, print settings, a
    /// slicer's own metadata -- and the one it wants is found by name.
    pub fn open(data: &'a [u8]) -> Result<Archive<'a>, String> {
        let directory = end_of_central_directory(data)?;
        let count = u16::from_le_bytes([data[directory + 10], data[directory + 11]]) as usize;
        let mut at = u32::from_le_bytes([
            data[directory + 16],
            data[directory + 17],
            data[directory + 18],
            data[directory + 19],
        ]) as usize;
        let mut entries = Vec::with_capacity(count);
        for _ in 0..count {
            let header = data.get(at..at + 46).ok_or_else(|| "the archive's file list is cut short".to_string())?;
            if header[0..4] != [0x50, 0x4b, 0x01, 0x02] {
                return Err("the archive's file list is not where it says it is".into());
            }
            let read16 = |at: usize| u16::from_le_bytes([header[at], header[at + 1]]) as usize;
            let read32 =
                |at: usize| u32::from_le_bytes([header[at], header[at + 1], header[at + 2], header[at + 3]]) as usize;
            let method = read16(10);
            let compressed_size = read32(20);
            let uncompressed_size = read32(24);
            let name_length = read16(28);
            let extra_length = read16(30);
            let comment_length = read16(32);
            let offset = read32(42);
            let name_at = at + 46;
            let name = data
                .get(name_at..name_at + name_length)
                .ok_or_else(|| "an entry's name runs past the end of the archive".to_string())?;
            entries.push(Entry {
                name: String::from_utf8_lossy(name).to_string(),
                stored: method == 0,
                compressed_size,
                uncompressed_size,
                offset,
            });
            at = name_at + name_length + extra_length + comment_length;
        }
        Ok(Archive { data, entries })
    }

    /// Every part's name, for saying what an archive holds when the one being
    /// looked for is not in it.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().map(|entry| entry.name.as_str())
    }

    /// The bytes of the first part whose name satisfies `wanted`, decompressed.
    pub fn read_by(&self, wanted: impl Fn(&str) -> bool) -> Option<Result<Vec<u8>, String>> {
        let entry = self.entries.iter().find(|entry| wanted(&entry.name))?;
        Some(self.read(entry))
    }

    fn read(&self, entry: &Entry) -> Result<Vec<u8>, String> {
        let header = self
            .data
            .get(entry.offset..entry.offset + 30)
            .ok_or_else(|| format!("{}: the entry's header is not in the archive", entry.name))?;
        if header[0..4] != [0x50, 0x4b, 0x03, 0x04] {
            return Err(format!("{}: the entry's header is not where the file list says", entry.name));
        }
        let name_length = u16::from_le_bytes([header[26], header[27]]) as usize;
        let extra_length = u16::from_le_bytes([header[28], header[29]]) as usize;
        let at = entry.offset + 30 + name_length + extra_length;
        let body = self
            .data
            .get(at..at + entry.compressed_size)
            .ok_or_else(|| format!("{}: the entry's data is cut short", entry.name))?;
        if entry.stored {
            Ok(body.to_vec())
        } else {
            inflate(body, entry.uncompressed_size).map_err(|why| format!("{}: {why}", entry.name))
        }
    }
}

/// Find the end-of-central-directory record, which is at the end of the file
/// unless the archive carries a comment -- so it is searched for backwards,
/// over as much as a comment can be.
fn end_of_central_directory(data: &[u8]) -> Result<usize, String> {
    const SIGNATURE: [u8; 4] = [0x50, 0x4b, 0x05, 0x06];
    if data.len() < 22 {
        return Err("the file is too short to be an archive".into());
    }
    let earliest = data.len().saturating_sub(22 + u16::MAX as usize);
    for at in (earliest..=data.len() - 22).rev() {
        if data[at..at + 4] == SIGNATURE {
            return Ok(at);
        }
    }
    Err("the file has no archive index, so it is not a 3MF package".into())
}
