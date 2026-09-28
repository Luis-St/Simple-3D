//! A dynamic block's own code tables and the run-length coded header describing them.

use super::*;

/// A block's own code tables and the header describing them.
pub(super) struct DynamicTables {
    pub(super) literals: Codes,
    pub(super) distances: Codes,
    /// How many of each alphabet is written, trailing unused symbols dropped.
    literal_count: usize,
    distance_count: usize,
    /// Both length sequences concatenated and run-length encoded, as (symbol, extra value) pairs.
    encoded: Vec<(u8, u8)>,
    code_lengths: Codes,
    written_code_lengths: usize,
}

impl DynamicTables {
    pub(super) fn new(literal_lengths: &[u8], distance_lengths: &[u8]) -> DynamicTables {
        let literal_count = literal_lengths.iter().rposition(|&l| l > 0).map_or(257, |at| (at + 1).max(257));
        let distance_count = distance_lengths.iter().rposition(|&l| l > 0).map_or(1, |at| at + 1);

        let mut sequence: Vec<u8> = Vec::with_capacity(literal_count + distance_count);
        sequence.extend_from_slice(&literal_lengths[..literal_count]);
        sequence.extend_from_slice(&distance_lengths[..distance_count]);
        let encoded = run_length_encode(&sequence);

        let mut code_length_freq = vec![0u32; 19];
        for (symbol, _) in &encoded {
            code_length_freq[*symbol as usize] += 1;
        }
        let code_lengths = canonical(huffman_lengths(&code_length_freq, MAX_CODE_LENGTH_BITS));
        // Trailing zero entries of the permuted order are not written; four is the minimum.
        let written_code_lengths =
            CODE_LENGTH_ORDER.iter().rposition(|&at| code_lengths.lengths[at] > 0).map_or(4, |at| (at + 1).max(4));

        DynamicTables {
            literals: canonical(literal_lengths.to_vec()),
            distances: canonical(distance_lengths.to_vec()),
            literal_count,
            distance_count,
            encoded,
            code_lengths,
            written_code_lengths,
        }
    }

    /// The cost of describing these tables, to decide whether they are worth writing.
    pub(super) fn header_bits(&self) -> usize {
        let mut bits = 3 + 5 + 5 + 4 + self.written_code_lengths * 3;
        for (symbol, _) in &self.encoded {
            bits += self.code_lengths.lengths[*symbol as usize] as usize;
            bits += match symbol {
                16 => 2,
                17 => 3,
                18 => 7,
                _ => 0,
            };
        }
        bits
    }

    pub(super) fn write_header(&self, out: &mut Bits) {
        out.push(self.literal_count as u32 - 257, 5);
        out.push(self.distance_count as u32 - 1, 5);
        out.push(self.written_code_lengths as u32 - 4, 4);
        for &at in CODE_LENGTH_ORDER.iter().take(self.written_code_lengths) {
            out.push(self.code_lengths.lengths[at] as u32, 3);
        }
        for &(symbol, extra) in &self.encoded {
            out.push_code(&self.code_lengths, symbol as usize);
            match symbol {
                16 => out.push(extra as u32, 2),
                17 => out.push(extra as u32, 3),
                18 => out.push(extra as u32, 7),
                _ => {}
            }
        }
    }
}

/// Run-length encode code lengths with symbols 16, 17 and 18.
fn run_length_encode(sequence: &[u8]) -> Vec<(u8, u8)> {
    let mut out = Vec::new();
    let mut at = 0;
    while at < sequence.len() {
        let value = sequence[at];
        let mut run = 1;
        while at + run < sequence.len() && sequence[at + run] == value {
            run += 1;
        }
        if value == 0 {
            while run >= 11 {
                let take = run.min(138);
                out.push((18u8, (take - 11) as u8));
                run -= take;
                at += take;
            }
            while run >= 3 {
                let take = run.min(10);
                out.push((17u8, (take - 3) as u8));
                run -= take;
                at += take;
            }
            for _ in 0..run {
                out.push((0u8, 0u8));
                at += 1;
            }
        } else {
            // Symbol 16 repeats the previous value, so the first one must be written out.
            out.push((value, 0u8));
            at += 1;
            run -= 1;
            while run >= 3 {
                let take = run.min(6);
                out.push((16u8, (take - 3) as u8));
                run -= take;
                at += take;
            }
            for _ in 0..run {
                out.push((value, 0u8));
                at += 1;
            }
        }
    }
    out
}
