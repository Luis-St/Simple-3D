//! A raw DEFLATE decoder (RFC 1951), for the compressed parts of a 3MF.
//!
//! Hand-written to keep the binary dependency-free. Decodes bit by bit against canonical tables
//! rather than via a full lookup table: model parts are small, and this stays easy to check
//! against the RFC.

/// The maximum Huffman code length, fixed by DEFLATE at 15.
const MAX_BITS: usize = 15;

/// Bits out of a byte slice, least significant first as DEFLATE packs them.
struct Bits<'a> {
    data: &'a [u8],
    /// The next bit to read, from the start of `data`.
    at: usize,
}

impl<'a> Bits<'a> {
    fn new(data: &'a [u8]) -> Bits<'a> {
        Bits { data, at: 0 }
    }

    fn bit(&mut self) -> Result<u32, String> {
        let byte = self.data.get(self.at / 8).ok_or_else(|| "the compressed data ends mid-symbol".to_string())?;
        let bit = (*byte >> (self.at % 8)) & 1;
        self.at += 1;
        Ok(bit as u32)
    }

    /// `count` bits as a number, first bit least significant, as DEFLATE writes fixed fields.
    fn bits(&mut self, count: usize) -> Result<u32, String> {
        let mut value = 0;
        for i in 0..count {
            value |= self.bit()? << i;
        }
        Ok(value)
    }

    /// Skip to the next byte boundary, before a stored block.
    fn align(&mut self) {
        self.at = self.at.div_ceil(8) * 8;
    }

    fn byte_position(&self) -> usize {
        self.at / 8
    }
}

/// A canonical Huffman table: code counts per length, and the symbols in code order.
struct Huffman {
    counts: [u16; MAX_BITS + 1],
    symbols: Vec<u16>,
}

impl Huffman {
    /// Build the table from one code length per symbol; zero means no code.
    fn new(lengths: &[u8]) -> Huffman {
        let mut counts = [0u16; MAX_BITS + 1];
        for &length in lengths {
            counts[length as usize] += 1;
        }
        // Where each length's symbols start; length 0 takes no room.
        let mut offset = [0u16; MAX_BITS + 2];
        for length in 1..=MAX_BITS {
            offset[length + 1] = offset[length] + counts[length];
        }
        let mut symbols = vec![0u16; offset[MAX_BITS + 1] as usize];
        for (symbol, &length) in lengths.iter().enumerate() {
            if length != 0 {
                symbols[offset[length as usize] as usize] = symbol as u16;
                offset[length as usize] += 1;
            }
        }
        Huffman { counts, symbols }
    }

    /// Read one symbol, accumulating the code most significant bit first (unlike fixed fields).
    fn decode(&self, bits: &mut Bits<'_>) -> Result<u16, String> {
        let mut code = 0i32;
        let mut first = 0i32;
        let mut index = 0i32;
        for length in 1..=MAX_BITS {
            code |= bits.bit()? as i32;
            let count = self.counts[length] as i32;
            if code - count < first {
                return Ok(self.symbols[(index + (code - first)) as usize]);
            }
            index += count;
            first = (first + count) << 1;
            code <<= 1;
        }
        Err("the compressed data holds a code no table answers".into())
    }
}

/// The length each length symbol stands for, and its extra bits.
const LENGTH_BASE: [u16; 29] =
    [3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131, 163, 195, 227, 258];
const LENGTH_EXTRA: [u8; 29] = [0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0];

/// The same for distance symbols.
const DIST_BASE: [u16; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537, 2049, 3073, 4097, 6145,
    8193, 12289, 16385, 24577,
];
const DIST_EXTRA: [u8; 30] =
    [0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13, 13];

/// Decompress a raw DEFLATE stream, as a zip entry holds.
///
/// `expected` is the archive's promised size, used to reserve output and to refuse streams that
/// write far more, guarding against decompression bombs.
pub fn inflate(data: &[u8], expected: usize) -> Result<Vec<u8>, String> {
    // Some slack: the limit guards against lies by orders of magnitude, not inexact sizes.
    let limit = expected.saturating_mul(2).max(1 << 16);
    let mut out: Vec<u8> = Vec::with_capacity(expected.min(1 << 22));
    let mut bits = Bits::new(data);
    loop {
        let last = bits.bit()? == 1;
        match bits.bits(2)? {
            0 => stored(&mut bits, &mut out)?,
            1 => block(&mut bits, &mut out, &fixed_literals(), &fixed_distances(), limit)?,
            2 => {
                let (literals, distances) = dynamic_tables(&mut bits)?;
                block(&mut bits, &mut out, &literals, &distances, limit)?
            }
            _ => return Err("the compressed data names a block type DEFLATE does not have".into()),
        }
        if last {
            return Ok(out);
        }
    }
}

/// A stored block: byte-aligned length, its one's complement, then the bytes.
fn stored(bits: &mut Bits<'_>, out: &mut Vec<u8>) -> Result<(), String> {
    bits.align();
    let at = bits.byte_position();
    let header = bits.data.get(at..at + 4).ok_or_else(|| "a stored block is cut short".to_string())?;
    let length = u16::from_le_bytes([header[0], header[1]]) as usize;
    let check = u16::from_le_bytes([header[2], header[3]]);
    if check != !(length as u16) {
        return Err("a stored block's length does not match its check field".into());
    }
    let start = at + 4;
    let body = bits.data.get(start..start + length).ok_or_else(|| "a stored block is cut short".to_string())?;
    out.extend_from_slice(body);
    bits.at = (start + length) * 8;
    Ok(())
}

/// A Huffman-coded block of literals and length/distance back-references.
fn block(
    bits: &mut Bits<'_>,
    out: &mut Vec<u8>,
    literals: &Huffman,
    distances: &Huffman,
    limit: usize,
) -> Result<(), String> {
    loop {
        let symbol = literals.decode(bits)? as usize;
        if symbol < 256 {
            out.push(symbol as u8);
        } else if symbol == 256 {
            return Ok(());
        } else {
            let index = symbol - 257;
            if index >= LENGTH_BASE.len() {
                return Err("the compressed data names a length symbol DEFLATE does not have".into());
            }
            let length = LENGTH_BASE[index] as usize + bits.bits(LENGTH_EXTRA[index] as usize)? as usize;
            let symbol = distances.decode(bits)? as usize;
            if symbol >= DIST_BASE.len() {
                return Err("the compressed data names a distance symbol DEFLATE does not have".into());
            }
            let distance = DIST_BASE[symbol] as usize + bits.bits(DIST_EXTRA[symbol] as usize)? as usize;
            if distance > out.len() {
                return Err("the compressed data refers back past the start of the stream".into());
            }
            // Byte by byte, since the run may overlap itself.
            for from in (out.len() - distance..).take(length) {
                let byte = out[from];
                out.push(byte);
            }
        }
        if out.len() > limit {
            return Err("the compressed data expands to far more than the archive says it holds".into());
        }
    }
}

/// The fixed literal/length table (RFC 1951 section 3.2.6).
fn fixed_literals() -> Huffman {
    let mut lengths = [0u8; 288];
    for (symbol, length) in lengths.iter_mut().enumerate() {
        *length = match symbol {
            0..=143 => 8,
            144..=255 => 9,
            256..=279 => 7,
            _ => 8,
        };
    }
    Huffman::new(&lengths)
}

/// The fixed distance table: thirty-two five-bit codes.
fn fixed_distances() -> Huffman {
    Huffman::new(&[5u8; 30])
}

/// Read a dynamic block's two tables, themselves coded by a code-length table.
fn dynamic_tables(bits: &mut Bits<'_>) -> Result<(Huffman, Huffman), String> {
    // The order the code-length alphabet's own lengths are written in.
    const ORDER: [usize; 19] = [16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15];
    let literal_count = bits.bits(5)? as usize + 257;
    let distance_count = bits.bits(5)? as usize + 1;
    let code_count = bits.bits(4)? as usize + 4;
    if literal_count > 286 || distance_count > 30 {
        return Err("the compressed data declares more codes than DEFLATE allows".into());
    }
    let mut code_lengths = [0u8; 19];
    for &position in ORDER.iter().take(code_count) {
        code_lengths[position] = bits.bits(3)? as u8;
    }
    let code_table = Huffman::new(&code_lengths);

    let mut lengths = vec![0u8; literal_count + distance_count];
    let mut at = 0;
    while at < lengths.len() {
        let symbol = code_table.decode(bits)?;
        match symbol {
            0..=15 => {
                lengths[at] = symbol as u8;
                at += 1;
            }
            // Repeat the previous length 3 to 6 times.
            16 => {
                if at == 0 {
                    return Err("the compressed data repeats a code length before there is one".into());
                }
                let previous = lengths[at - 1];
                let repeat = 3 + bits.bits(2)? as usize;
                fill(&mut lengths, &mut at, previous, repeat)?;
            }
            // A run of zeroes, short then long.
            17 => {
                let repeat = 3 + bits.bits(3)? as usize;
                fill(&mut lengths, &mut at, 0, repeat)?;
            }
            18 => {
                let repeat = 11 + bits.bits(7)? as usize;
                fill(&mut lengths, &mut at, 0, repeat)?;
            }
            _ => return Err("the compressed data names a code-length symbol DEFLATE does not have".into()),
        }
    }
    Ok((Huffman::new(&lengths[..literal_count]), Huffman::new(&lengths[literal_count..])))
}

fn fill(lengths: &mut [u8], at: &mut usize, value: u8, repeat: usize) -> Result<(), String> {
    if *at + repeat > lengths.len() {
        return Err("the compressed data's code lengths run past the alphabet".into());
    }
    for _ in 0..repeat {
        lengths[*at] = value;
        *at += 1;
    }
    Ok(())
}
