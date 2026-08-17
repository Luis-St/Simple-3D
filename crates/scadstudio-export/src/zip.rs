//! A minimal store-only ZIP writer, just enough for the OPC package a 3MF file
//! is (spec section 9).
//!
//! Hand-written rather than pulled in as a dependency: the whole archive is
//! three small XML parts, "stored" is a valid ZIP compression method that every
//! reader supports, and the alternative is a compression crate and its
//! transitive tree inside a binary that must stay self-contained. The 3MF
//! specification permits stored entries; the resulting file is larger than a
//! deflated one, which for three XML parts is not worth a dependency.

/// CRC-32 (IEEE), computed with a table built on first use.
fn crc32(data: &[u8]) -> u32 {
    let table = crc_table();
    let mut crc = 0xffff_ffffu32;
    for &byte in data {
        crc = table[((crc ^ byte as u32) & 0xff) as usize] ^ (crc >> 8);
    }
    !crc
}

fn crc_table() -> &'static [u32; 256] {
    use std::sync::OnceLock;
    static TABLE: OnceLock<[u32; 256]> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut table = [0u32; 256];
        for (i, entry) in table.iter_mut().enumerate() {
            let mut c = i as u32;
            for _ in 0..8 {
                c = if c & 1 != 0 { 0xedb8_8320 ^ (c >> 1) } else { c >> 1 };
            }
            *entry = c;
        }
        table
    })
}

struct Entry {
    name: String,
    crc: u32,
    size: u32,
    offset: u32,
}

pub struct ZipWriter {
    buffer: Vec<u8>,
    entries: Vec<Entry>,
}

impl Default for ZipWriter {
    fn default() -> Self {
        ZipWriter::new()
    }
}

impl ZipWriter {
    pub fn new() -> ZipWriter {
        ZipWriter { buffer: Vec::new(), entries: Vec::new() }
    }

    pub fn add(&mut self, name: &str, data: &[u8]) {
        let offset = self.buffer.len() as u32;
        let crc = crc32(data);
        let size = data.len() as u32;

        self.buffer.extend_from_slice(&0x0403_4b50u32.to_le_bytes()); // local file header
        self.buffer.extend_from_slice(&20u16.to_le_bytes()); // version needed
        self.buffer.extend_from_slice(&0u16.to_le_bytes()); // flags
        self.buffer.extend_from_slice(&0u16.to_le_bytes()); // method: stored
        // A fixed timestamp, so exporting the same scene twice produces
        // byte-identical files -- the same reason the evaluator is deterministic.
        self.buffer.extend_from_slice(&0u16.to_le_bytes()); // time
        self.buffer.extend_from_slice(&0x21u16.to_le_bytes()); // date: 1980-01-01
        self.buffer.extend_from_slice(&crc.to_le_bytes());
        self.buffer.extend_from_slice(&size.to_le_bytes()); // compressed size
        self.buffer.extend_from_slice(&size.to_le_bytes()); // uncompressed size
        self.buffer.extend_from_slice(&(name.len() as u16).to_le_bytes());
        self.buffer.extend_from_slice(&0u16.to_le_bytes()); // extra length
        self.buffer.extend_from_slice(name.as_bytes());
        self.buffer.extend_from_slice(data);

        self.entries.push(Entry { name: name.to_string(), crc, size, offset });
    }

    pub fn finish(mut self) -> Vec<u8> {
        let cd_offset = self.buffer.len() as u32;
        for entry in &self.entries {
            self.buffer.extend_from_slice(&0x0201_4b50u32.to_le_bytes()); // central directory
            self.buffer.extend_from_slice(&20u16.to_le_bytes()); // version made by
            self.buffer.extend_from_slice(&20u16.to_le_bytes()); // version needed
            self.buffer.extend_from_slice(&0u16.to_le_bytes()); // flags
            self.buffer.extend_from_slice(&0u16.to_le_bytes()); // method
            self.buffer.extend_from_slice(&0u16.to_le_bytes()); // time
            self.buffer.extend_from_slice(&0x21u16.to_le_bytes()); // date
            self.buffer.extend_from_slice(&entry.crc.to_le_bytes());
            self.buffer.extend_from_slice(&entry.size.to_le_bytes());
            self.buffer.extend_from_slice(&entry.size.to_le_bytes());
            self.buffer.extend_from_slice(&(entry.name.len() as u16).to_le_bytes());
            self.buffer.extend_from_slice(&0u16.to_le_bytes()); // extra
            self.buffer.extend_from_slice(&0u16.to_le_bytes()); // comment
            self.buffer.extend_from_slice(&0u16.to_le_bytes()); // disk number
            self.buffer.extend_from_slice(&0u16.to_le_bytes()); // internal attrs
            self.buffer.extend_from_slice(&0u32.to_le_bytes()); // external attrs
            self.buffer.extend_from_slice(&entry.offset.to_le_bytes());
            self.buffer.extend_from_slice(entry.name.as_bytes());
        }
        let cd_size = self.buffer.len() as u32 - cd_offset;
        let count = self.entries.len() as u16;

        self.buffer.extend_from_slice(&0x0605_4b50u32.to_le_bytes()); // end of central directory
        self.buffer.extend_from_slice(&0u16.to_le_bytes()); // this disk
        self.buffer.extend_from_slice(&0u16.to_le_bytes()); // disk with CD
        self.buffer.extend_from_slice(&count.to_le_bytes());
        self.buffer.extend_from_slice(&count.to_le_bytes());
        self.buffer.extend_from_slice(&cd_size.to_le_bytes());
        self.buffer.extend_from_slice(&cd_offset.to_le_bytes());
        self.buffer.extend_from_slice(&0u16.to_le_bytes()); // comment length
        self.buffer
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc32_matches_the_known_check_value() {
        // The IEEE CRC-32 of "123456789" is 0xCBF43926.
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
        assert_eq!(crc32(b""), 0);
    }

    #[test]
    fn the_archive_has_the_structure_a_reader_looks_for() {
        let mut zip = ZipWriter::new();
        zip.add("[Content_Types].xml", b"<Types/>");
        zip.add("3D/3dmodel.model", b"<model/>");
        let bytes = zip.finish();

        assert_eq!(&bytes[0..4], &0x0403_4b50u32.to_le_bytes());
        // The end-of-central-directory record is the last 22 bytes when there is
        // no archive comment, and a reader finds everything else from it.
        let eocd = &bytes[bytes.len() - 22..];
        assert_eq!(&eocd[0..4], &0x0605_4b50u32.to_le_bytes());
        assert_eq!(u16::from_le_bytes([eocd[10], eocd[11]]), 2, "entry count");
        let cd_offset = u32::from_le_bytes([eocd[16], eocd[17], eocd[18], eocd[19]]) as usize;
        assert_eq!(&bytes[cd_offset..cd_offset + 4], &0x0201_4b50u32.to_le_bytes());
        // Stored, so the payloads appear verbatim.
        assert!(bytes.windows(8).any(|w| w == b"<model/>"));
    }

    #[test]
    fn the_same_content_produces_the_same_bytes() {
        let build = || {
            let mut zip = ZipWriter::new();
            zip.add("a.xml", b"<a/>");
            zip.finish()
        };
        assert_eq!(build(), build());
    }
}
