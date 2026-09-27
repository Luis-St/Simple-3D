//! A DEFLATE encoder (RFC 1951), so a 3MF is written compressed.
//!
//! Hand-written to keep the binary dependency-free. The model part grows with the model and is
//! highly repetitive, so compression matters for sharing and uploading. Implements LZ77 with hash
//! chains and one-step lazy matching, and picks per block the smallest of dynamic Huffman, fixed
//! Huffman or stored. Dynamic tables give about 4.3x versus 3.3x for fixed on a model part.
//! Tests round-trip through [`simple3d_import`]'s decoder.

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

// -- LZ77 -------------------------------------------------------------------

/// Turn `data` into literals and back-references.
///
/// Every position is entered into the hash chain, including those inside a match; skipping
/// them costs noticeable ratio.
fn tokenize(data: &[u8]) -> Vec<Token> {
    let mut tokens = Vec::with_capacity(data.len() / 4 + 16);
    let mut head = vec![u32::MAX; HASH_SIZE];
    // One slot per window position rather than per byte, so a 20MB part does not need 80MB of
    // chain. Older positions share slots and are rejected by the window check in `longest_match`.
    let mut prev = vec![u32::MAX; WINDOW];

    let mut at = 0usize;
    // The lazy step: a match held back while checking whether the next position has a better one.
    let mut held: Option<(usize, usize)> = None;
    while at < data.len() {
        let (mut length, mut distance) = (0usize, 0usize);
        if at + MIN_MATCH <= data.len() {
            let key = hash(&data[at..]);
            let (found_length, found_distance) = longest_match(data, at, key, &head, &prev);
            length = found_length;
            distance = found_distance;
            insert(&mut head, &mut prev, key, at);
        }

        match held.take() {
            // Take the longer of the held and current match; the held one wins ties, being nearer.
            Some((held_length, held_distance)) => {
                if length > held_length {
                    tokens.push(Token::Literal(data[at - 1]));
                    held = Some((length, distance));
                    at += 1;
                } else {
                    tokens.push(Token::Match { length: held_length as u16, distance: held_distance as u16 });
                    // The first two covered positions are already entered, so enter only the rest; entering one
                    // twice would make its chain point at itself.
                    for skip in (at + 1)..(at - 1 + held_length).min(data.len()) {
                        if skip + MIN_MATCH <= data.len() {
                            let key = hash(&data[skip..]);
                            insert(&mut head, &mut prev, key, skip);
                        }
                    }
                    at = at - 1 + held_length;
                }
            }
            None => {
                if length >= MIN_MATCH {
                    held = Some((length, distance));
                    at += 1;
                } else {
                    tokens.push(Token::Literal(data[at]));
                    at += 1;
                }
            }
        }
    }
    // A match held back at the end of the input is taken as it stands.
    if let Some((length, distance)) = held {
        tokens.push(Token::Match { length: length as u16, distance: distance as u16 });
    }
    tokens
}

fn hash(bytes: &[u8]) -> usize {
    // Multiply-and-shift spreads XML's few characters far better than the shift-and-xor it replaced.
    let key = (bytes[0] as u32) | ((bytes[1] as u32) << 8) | ((bytes[2] as u32) << 16);
    ((key.wrapping_mul(0x9E37_79B1)) >> (32 - HASH_BITS)) as usize
}

fn insert(head: &mut [u32], prev: &mut [u32], key: usize, at: usize) {
    prev[at & (WINDOW - 1)] = head[key];
    head[key] = at as u32;
}

/// The longest earlier run within the window matching at `at`, and its distance; `(0, 0)` if none.
fn longest_match(data: &[u8], at: usize, key: usize, head: &[u32], prev: &[u32]) -> (usize, usize) {
    let limit = (data.len() - at).min(MAX_MATCH);
    if limit < MIN_MATCH {
        return (0, 0);
    }
    let earliest = at.saturating_sub(WINDOW);
    let mut best_length = 0usize;
    let mut best_distance = 0usize;
    let mut candidate = head[key];
    for _ in 0..MAX_CHAIN {
        if candidate == u32::MAX {
            break;
        }
        let start = candidate as usize;
        if start < earliest {
            break;
        }
        // Check the byte past the current best first; most candidates fail here.
        if best_length == 0 || data[start + best_length] == data[at + best_length] {
            let mut length = 0;
            while length < limit && data[start + length] == data[at + length] {
                length += 1;
            }
            if length > best_length {
                best_length = length;
                best_distance = at - start;
                if length >= GOOD_ENOUGH || length == limit {
                    break;
                }
            }
        }
        candidate = prev[start & (WINDOW - 1)];
    }
    if best_length >= MIN_MATCH {
        (best_length, best_distance)
    } else {
        (0, 0)
    }
}

// -- blocks -----------------------------------------------------------------

/// Write one block, in whichever of the three forms is smallest.
fn emit_block(out: &mut Bits, tokens: &[Token], bytes: &[u8], last: bool) {
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

// -- Huffman ----------------------------------------------------------------

/// A Huffman table: a code and length per symbol, the code already bit-reversed for writing.
struct Codes {
    lengths: Vec<u8>,
    codes: Vec<u16>,
}

/// Code lengths for `freqs`, none longer than `limit`.
///
/// Builds an ordinary Huffman tree, then clamps over-long lengths and repairs the table until it
/// is exactly full. Cheaper to implement than package-merge and within a fraction of a percent.
fn huffman_lengths(freqs: &[u32], limit: usize) -> Vec<u8> {
    let mut lengths = vec![0u8; freqs.len()];
    let used: Vec<usize> = (0..freqs.len()).filter(|&symbol| freqs[symbol] > 0).collect();
    match used.len() {
        // Nothing to code, e.g. a literals-only block's distance alphabet.
        0 => return lengths,
        // A single symbol still needs a one-bit code; the incomplete table is accepted by every decoder
        // and written by every other encoder.
        1 => {
            lengths[used[0]] = 1;
            return lengths;
        }
        _ => {}
    }

    // The tree, as a heap of (weight, tie-breaker, node).
    let mut nodes: Vec<(u64, usize, usize)> = Vec::with_capacity(used.len() * 2);
    // Children of each internal node, and the depth pass that follows.
    let mut children: Vec<(usize, usize)> = Vec::new();
    for (order, &symbol) in used.iter().enumerate() {
        nodes.push((freqs[symbol] as u64, order, symbol));
    }
    let leaves = freqs.len();
    let mut heap: std::collections::BinaryHeap<std::cmp::Reverse<(u64, usize, usize)>> =
        nodes.into_iter().map(std::cmp::Reverse).collect();
    let mut order = used.len();
    while heap.len() > 1 {
        let std::cmp::Reverse((weight_a, _, a)) = heap.pop().expect("two or more nodes");
        let std::cmp::Reverse((weight_b, _, b)) = heap.pop().expect("two or more nodes");
        children.push((a, b));
        let node = leaves + children.len() - 1;
        heap.push(std::cmp::Reverse((weight_a + weight_b, order, node)));
        order += 1;
    }
    let std::cmp::Reverse((_, _, root)) = heap.pop().expect("one node is left");

    // Iterative, since a degenerate tree can be 285 deep.
    let mut depth = vec![0usize; leaves + children.len()];
    let mut stack = vec![(root, 0usize)];
    while let Some((node, at)) = stack.pop() {
        if node < leaves {
            depth[node] = at;
            lengths[node] = at.min(limit) as u8;
        } else {
            let (a, b) = children[node - leaves];
            stack.push((a, at + 1));
            stack.push((b, at + 1));
        }
    }

    // Clamping may over-fill the table. Repair: remove a code at the longest length, split a
    // shorter one to replace it, and repeat until exactly full.
    let mut counts = vec![0u32; limit + 2];
    for &symbol in &used {
        counts[lengths[symbol] as usize] += 1;
    }
    let full = 1u64 << limit;
    let mut total: u64 = (1..=limit).map(|length| (counts[length] as u64) << (limit - length)).sum();
    while total > full {
        counts[limit] -= 1;
        for length in (1..limit).rev() {
            if counts[length] != 0 {
                counts[length] -= 1;
                counts[length + 1] += 2;
                break;
            }
        }
        total -= 1;
    }
    // Under-filling cannot happen: the tree is complete and clamping only over-fills.
    debug_assert_eq!(total, full, "the repaired table is not a complete code");

    // Assign the longest lengths to the least frequent symbols, so the table stays optimal.
    let mut by_frequency = used.clone();
    by_frequency.sort_by_key(|&symbol| (std::cmp::Reverse(freqs[symbol]), symbol));
    let mut at = 0;
    for (length, &count) in counts.iter().enumerate().take(limit + 1).skip(1) {
        for _ in 0..count {
            lengths[by_frequency[at]] = length as u8;
            at += 1;
        }
    }
    debug_assert_eq!(at, used.len(), "every symbol that occurs must get a code");
    lengths
}

/// Canonical codes for a set of lengths.
fn canonical(lengths: Vec<u8>) -> Codes {
    let longest = lengths.iter().copied().max().unwrap_or(0) as usize;
    let mut counts = vec![0u16; longest + 2];
    for &length in &lengths {
        if length > 0 {
            counts[length as usize] += 1;
        }
    }
    let mut next = vec![0u16; longest + 2];
    let mut code = 0u16;
    for length in 1..=longest {
        code = (code + counts[length - 1]) << 1;
        next[length] = code;
    }
    let mut codes = vec![0u16; lengths.len()];
    for (symbol, &length) in lengths.iter().enumerate() {
        if length > 0 {
            codes[symbol] = next[length as usize];
            next[length as usize] += 1;
        }
    }
    Codes { lengths, codes }
}

fn fixed_tables() -> (Codes, Codes) {
    let mut literals = vec![0u8; 288];
    for (symbol, length) in literals.iter_mut().enumerate() {
        *length = match symbol {
            0..=143 => 8,
            144..=255 => 9,
            256..=279 => 7,
            _ => 8,
        };
    }
    (canonical(literals), canonical(vec![5u8; 30]))
}

/// A block's own code tables and the header describing them.
struct DynamicTables {
    literals: Codes,
    distances: Codes,
    /// How many of each alphabet is written, trailing unused symbols dropped.
    literal_count: usize,
    distance_count: usize,
    /// Both length sequences concatenated and run-length encoded, as (symbol, extra value) pairs.
    encoded: Vec<(u8, u8)>,
    code_lengths: Codes,
    written_code_lengths: usize,
}

impl DynamicTables {
    fn new(literal_lengths: &[u8], distance_lengths: &[u8]) -> DynamicTables {
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
    fn header_bits(&self) -> usize {
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

    fn write_header(&self, out: &mut Bits) {
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

// -- bits -------------------------------------------------------------------

/// Bits out, least significant first as DEFLATE packs them; Huffman codes are reversed by
/// [`Bits::push_code`].
struct Bits {
    out: Vec<u8>,
    accumulator: u32,
    held: u32,
}

impl Bits {
    fn new(capacity: usize) -> Bits {
        Bits { out: Vec::with_capacity(capacity), accumulator: 0, held: 0 }
    }

    fn push(&mut self, value: u32, count: usize) {
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

    fn push_code(&mut self, table: &Codes, symbol: usize) {
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

    fn finish(mut self) -> Vec<u8> {
        self.align();
        self.out
    }
}
