//! A DEFLATE encoder (RFC 1951), so a 3MF is written compressed.
//!
//! Hand-written to keep the binary dependency-free. The model part grows with the model and is
//! highly repetitive, so compression matters for sharing and uploading. Implements LZ77 with hash
//! chains and one-step lazy matching, and picks per block the smallest of dynamic Huffman, fixed
//! Huffman or stored. Dynamic tables give about 4.3x versus 3.3x for fixed on a model part.
//! Tests round-trip through [`simple3d_import`]'s decoder.

mod emit;
mod huffman;
mod lz77;
mod tables;
use emit::*;
use huffman::*;
use lz77::*;
use tables::*;

/// The window a match may reach back over, fixed by DEFLATE at 32 KiB.
const WINDOW: usize = 32768;

/// The shortest and longest match, fixed by the format.
const MIN_MATCH: usize = 3;
const MAX_MATCH: usize = 258;

/// How many same-prefix positions are examined before taking the best match. The ratio is
/// nearly flat past this but the time is not: a prefix like `="0` chains through every vertex.
const MAX_CHAIN: usize = 192;

/// A match this long is taken without searching further back.
const GOOD_ENOUGH: usize = 128;

/// Tokens per block: long enough to pay for the table header, short enough for the statistics
/// to still fit the data.
const BLOCK_TOKENS: usize = 16384;

const HASH_BITS: usize = 15;
const HASH_SIZE: usize = 1 << HASH_BITS;

/// The length each length symbol stands for, and its extra bits.
const LENGTH_BASE: [u16; 29] =
    [3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131, 163, 195, 227, 258];
const LENGTH_EXTRA: [u8; 29] = [0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0];
const DIST_BASE: [u16; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537, 2049, 3073, 4097, 6145,
    8193, 12289, 16385, 24577,
];
const DIST_EXTRA: [u8; 30] =
    [0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13, 13];

/// The order the code-length alphabet's own lengths are written in.
const CODE_LENGTH_ORDER: [usize; 19] = [16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15];

/// The code length caps for the main alphabets and for the code-length alphabet.
const MAX_BITS: usize = 15;
const MAX_CODE_LENGTH_BITS: usize = 7;

/// A literal byte, or a run copied from earlier in the output.
#[derive(Clone, Copy)]
enum Token {
    Literal(u8),
    Match { length: u16, distance: u16 },
}

/// Compress `data` as a raw DEFLATE stream, as a zip entry holds.
pub(crate) fn deflate(data: &[u8]) -> Vec<u8> {
    let mut out = Bits::new(data.len() / 3 + 64);
    if data.is_empty() {
        // An empty entry still needs one final empty block; zip readers expect an end-of-block symbol.
        out.push(1, 1);
        out.push(1, 2);
        let (literals, _) = fixed_tables();
        out.push_code(&literals, 256);
        return out.finish();
    }

    let tokens = tokenize(data);
    // Tracks each block's byte range, so a stored block can find its bytes again.
    let mut at = 0;
    let mut consumed = 0usize;
    while at < tokens.len() {
        let end = (at + BLOCK_TOKENS).min(tokens.len());
        let block = &tokens[at..end];
        let bytes: usize = block
            .iter()
            .map(|token| match token {
                Token::Literal(_) => 1,
                Token::Match { length, .. } => *length as usize,
            })
            .sum();
        let last = end == tokens.len();
        emit_block(&mut out, block, &data[consumed..consumed + bytes], last);
        consumed += bytes;
        at = end;
    }
    out.finish()
}
