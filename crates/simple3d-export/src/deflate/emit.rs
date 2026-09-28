//! Blocks written in their smallest form, and the bit writer under them.

use super::*;

/// Write one block, in whichever of the three forms is smallest.
pub(super) fn emit_block(out: &mut Bits, tokens: &[Token], bytes: &[u8], last: bool) {
    let (literal_freq, distance_freq) = frequencies(tokens);

    let (fixed_literals, fixed_distances) = fixed_tables();
    let fixed_bits = 3 + token_bits(tokens, &fixed_literals, &fixed_distances);

    let literal_lengths = huffman_lengths(&literal_freq, MAX_BITS);
    let distance_lengths = huffman_lengths(&distance_freq, MAX_BITS);
    let dynamic = DynamicTables::new(&literal_lengths, &distance_lengths);
    let dynamic_bits = dynamic.header_bits() + token_bits(tokens, &dynamic.literals, &dynamic.distances);

    // Stored wins on already compressed data, such as a 3MF thumbnail.
    let stored_bits = 3 + 7 + 32 + bytes.len() * 8;

    if stored_bits <= fixed_bits.min(dynamic_bits) {
        emit_stored(out, bytes, last);
    } else if fixed_bits <= dynamic_bits {
        out.push(last as u32, 1);
        out.push(1, 2);
        emit_tokens(out, tokens, &fixed_literals, &fixed_distances);
    } else {
        out.push(last as u32, 1);
        out.push(2, 2);
        dynamic.write_header(out);
        emit_tokens(out, tokens, &dynamic.literals, &dynamic.distances);
    }
}

fn emit_stored(out: &mut Bits, bytes: &[u8], last: bool) {
    // Stored blocks hold at most 65535 bytes; only the final chunk of the final run is marked last.
    let mut chunks = bytes.chunks(u16::MAX as usize).peekable();
    if chunks.peek().is_none() {
        out.push(last as u32, 1);
        out.push(0, 2);
        out.align();
        out.push_bytes(&0u16.to_le_bytes());
        out.push_bytes(&u16::MAX.to_le_bytes());
        return;
    }
    while let Some(chunk) = chunks.next() {
        let final_chunk = last && chunks.peek().is_none();
        out.push(final_chunk as u32, 1);
        out.push(0, 2);
        out.align();
        out.push_bytes(&(chunk.len() as u16).to_le_bytes());
        out.push_bytes(&(!(chunk.len() as u16)).to_le_bytes());
        out.push_bytes(chunk);
    }
}

fn frequencies(tokens: &[Token]) -> (Vec<u32>, Vec<u32>) {
    let mut literals = vec![0u32; 286];
    let mut distances = vec![0u32; 30];
    for token in tokens {
        match token {
            Token::Literal(byte) => literals[*byte as usize] += 1,
            Token::Match { length, distance } => {
                literals[257 + length_symbol(*length)] += 1;
                distances[distance_symbol(*distance)] += 1;
            }
        }
    }
    // The end-of-block symbol, which every block writes exactly once.
    literals[256] += 1;
    (literals, distances)
}

/// The bits these tokens take under the given tables, used to choose the block type.
fn token_bits(tokens: &[Token], literals: &Codes, distances: &Codes) -> usize {
    let mut bits = literals.lengths[256] as usize;
    for token in tokens {
        match token {
            Token::Literal(byte) => bits += literals.lengths[*byte as usize] as usize,
            Token::Match { length, distance } => {
                let length_at = length_symbol(*length);
                let distance_at = distance_symbol(*distance);
                bits += literals.lengths[257 + length_at] as usize + LENGTH_EXTRA[length_at] as usize;
                bits += distances.lengths[distance_at] as usize + DIST_EXTRA[distance_at] as usize;
            }
        }
    }
    bits
}

fn emit_tokens(out: &mut Bits, tokens: &[Token], literals: &Codes, distances: &Codes) {
    for token in tokens {
        match token {
            Token::Literal(byte) => out.push_code(literals, *byte as usize),
            Token::Match { length, distance } => {
                let at = length_symbol(*length);
                out.push_code(literals, 257 + at);
                out.push(*length as u32 - LENGTH_BASE[at] as u32, LENGTH_EXTRA[at] as usize);
                let at = distance_symbol(*distance);
                out.push_code(distances, at);
                out.push(*distance as u32 - DIST_BASE[at] as u32, DIST_EXTRA[at] as usize);
            }
        }
    }
    out.push_code(literals, 256);
}

fn length_symbol(length: u16) -> usize {
    LENGTH_BASE.iter().rposition(|base| *base <= length).expect("a match is at least three long")
}

fn distance_symbol(distance: u16) -> usize {
    DIST_BASE.iter().rposition(|base| *base <= distance).expect("a match is at least one back")
}

/// Bits out, least significant first as DEFLATE packs them; Huffman codes are reversed by
/// [`Bits::push_code`].
pub(super) struct Bits {
    out: Vec<u8>,
    accumulator: u32,
    held: u32,
}

impl Bits {
    pub(super) fn new(capacity: usize) -> Bits {
        Bits { out: Vec::with_capacity(capacity), accumulator: 0, held: 0 }
    }

    pub(super) fn push(&mut self, value: u32, count: usize) {
        if count == 0 {
            return;
        }
        self.accumulator |= (value & ((1u32 << count) - 1)) << self.held;
        self.held += count as u32;
        while self.held >= 8 {
            self.out.push(self.accumulator as u8);
            self.accumulator >>= 8;
            self.held -= 8;
        }
    }

    pub(super) fn push_code(&mut self, table: &Codes, symbol: usize) {
        let length = table.lengths[symbol] as usize;
        debug_assert!(length > 0, "symbol {symbol} was written without a code");
        let code = table.codes[symbol];
        // Reversed: canonical codes are written most significant bit first.
        let mut reversed = 0u32;
        for bit in 0..length {
            reversed |= (((code >> bit) & 1) as u32) << (length - 1 - bit);
        }
        self.push(reversed, length);
    }

    fn align(&mut self) {
        if self.held > 0 {
            self.out.push(self.accumulator as u8);
            self.accumulator = 0;
            self.held = 0;
        }
    }

    fn push_bytes(&mut self, bytes: &[u8]) {
        debug_assert_eq!(self.held, 0, "bytes may only be written on a byte boundary");
        self.out.extend_from_slice(bytes);
    }

    pub(super) fn finish(mut self) -> Vec<u8> {
        self.align();
        self.out
    }
}
