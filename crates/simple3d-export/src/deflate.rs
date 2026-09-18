//! A DEFLATE encoder (RFC 1951), so a 3MF is written compressed.
//!
//! The package used to be written with every entry *stored*, on the grounds
//! that a 3MF is three small XML parts and a compression crate is a dependency
//! tree. Two of the parts are small. The third is not: `3D/3dmodel.model`
//! carries every vertex and every triangle as XML text, so it grows with the
//! model and is the most repetitive kind of data there is. A 100k-triangle
//! export is some 6MB stored, and the reason to compress it is not disk space
//! but that the file gets sent to somebody or uploaded to a printer.
//!
//! So the encoder is hand-written, for the reason the rest of the container is:
//! one self-contained binary with no dependency tree. What it implements:
//!
//! * **LZ77 with hash chains and one-step lazy matching**, which is what turns
//!   repeated XML into references rather than bytes.
//! * **Both Huffman block types.** A block is emitted with its own code tables
//!   when they pay for themselves, with the fixed tables when they do not, and
//!   stored when the data is incompressible -- whichever of the three is
//!   smallest, counted in bits before anything is written.
//!
//! On a 40k-vertex model part that is a factor of 4.3, against 3.3 for fixed
//! tables alone. The dynamic tables are the reason the second Huffman pass and
//! the length-limiting below exist at all; a third off the size of every file
//! the program writes was worth the page of code.
//!
//! The decoder that reads these files back is [`simple3d_import`]'s, and the
//! tests round-trip through it.

/// The window a match may reach back over, which DEFLATE fixes at 32 KiB.
const WINDOW: usize = 32768;

/// The shortest and longest run worth referring back to, also fixed by the
/// format.
const MIN_MATCH: usize = 3;
const MAX_MATCH: usize = 258;

/// How many earlier positions with the same three-byte prefix are examined
/// before taking the best match found so far.
///
/// The ratio is nearly flat past this and the time is not: the chain for a
/// three-byte prefix like `="0` in a model part is every vertex in the file.
const MAX_CHAIN: usize = 192;

/// A match this long is taken without looking further back. Anything longer is
/// a run of identical bytes, and the nearest one encodes in fewer bits.
const GOOD_ENOUGH: usize = 128;

/// How many tokens go into one block. The code tables are written once per
/// block, so a block wants to be long enough to pay for its header and short
/// enough that the statistics still describe the data in it.
const BLOCK_TOKENS: usize = 16384;

const HASH_BITS: usize = 15;
const HASH_SIZE: usize = 1 << HASH_BITS;

/// The length a length symbol stands for, and the extra bits it carries. The
/// same tables the decoder reads, in the direction that writes them.
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

/// What the literal/length alphabet and the distance alphabet are capped at,
/// and the shorter cap on the alphabet that describes them.
const MAX_BITS: usize = 15;
const MAX_CODE_LENGTH_BITS: usize = 7;

/// A literal byte, or a run copied from earlier in the output.
#[derive(Clone, Copy)]
enum Token {
    Literal(u8),
    Match { length: u16, distance: u16 },
}

/// Compress `data` as a raw DEFLATE stream: no zlib or gzip wrapper, which is
/// what a zip entry holds.
pub(crate) fn deflate(data: &[u8]) -> Vec<u8> {
    let mut out = Bits::new(data.len() / 3 + 64);
    if data.is_empty() {
        // An empty entry still has to be a valid stream: one final block, of
        // nothing. A zip reader that inflates it expects an end-of-block
        // symbol rather than no bytes at all.
        out.push(1, 1);
        out.push(1, 2);
        let (literals, _) = fixed_tables();
        out.push_code(&literals, 256);
        return out.finish();
    }

    let tokens = tokenize(data);
    // Where each block's tokens start and what of `data` they cover, so a
    // block that is better off stored can find its own bytes again.
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
/// Every position is entered into the hash chain, including the ones inside a
/// match: a later match may want to start there, and skipping them is what
/// makes a fast encoder a poor one.
fn tokenize(data: &[u8]) -> Vec<Token> {
    let mut tokens = Vec::with_capacity(data.len() / 4 + 16);
    let mut head = vec![u32::MAX; HASH_SIZE];
    // One slot per position in the window rather than one per byte of the
    // file: a 20MB model part would otherwise carry 80MB of chain. Positions
    // further back than the window share a slot, and a chain that reaches one
    // of those has already left the window and is stopped by the check in
    // `longest_match`.
    let mut prev = vec![u32::MAX; WINDOW];

    let mut at = 0usize;
    // The match found at `at` while deciding whether the one at `at + 1` is
    // better -- the "lazy" step, worth a few percent for the cost of holding
    // one match back.
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
            // A match was held back from the previous position. Take whichever
            // of the two is longer; the held one wins ties, being nearer.
            Some((held_length, held_distance)) => {
                if length > held_length {
                    tokens.push(Token::Literal(data[at - 1]));
                    held = Some((length, distance));
                    at += 1;
                } else {
                    tokens.push(Token::Match { length: held_length as u16, distance: held_distance as u16 });
                    // The match covers `at - 1` up to `at - 1 + held_length`.
                    // The first of those was entered when it was looked at and
                    // the second at the top of this turn, so the rest are
                    // entered here -- entering one twice would leave its chain
                    // pointing at itself.
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
    // Three bytes into fifteen: a multiply and a shift, which spreads the
    // handful of characters an XML file is made of across the whole table far
    // better than the shift-and-xor it replaced.
    let key = (bytes[0] as u32) | ((bytes[1] as u32) << 8) | ((bytes[2] as u32) << 16);
    ((key.wrapping_mul(0x9E37_79B1)) >> (32 - HASH_BITS)) as usize
}

fn insert(head: &mut [u32], prev: &mut [u32], key: usize, at: usize) {
    prev[at & (WINDOW - 1)] = head[key];
    head[key] = at as u32;
}

/// The longest run at `at` that also appears within the window behind it, and
/// how far back it is. `(0, 0)` when there is nothing worth referring to.
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
        // The byte one past the best match so far: if it does not agree, this
        // candidate cannot beat it, and most candidates fail here.
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

    // A stored block costs its bytes plus the padding to the next byte and a
    // four-byte length; it wins on data that is already compressed, which a
    // 3MF's thumbnail would be.
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
    // A stored block may hold 65535 bytes at most, so a long one is written as
    // several -- only the final block of the final run may say it is last.
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

/// How many bits these tokens take under a given pair of code tables, which is
/// what the choice between the block types is decided on.
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

/// A Huffman table as the encoder needs it: one code and one length per
/// symbol, with the code already in the order DEFLATE writes its bits.
struct Codes {
    lengths: Vec<u8>,
    codes: Vec<u16>,
}

/// Code lengths for `freqs`, none longer than `limit`.
///
/// The tree is built the ordinary way, by repeatedly joining the two least
/// frequent nodes, and is then *limited*: a length past the cap is clamped and
/// the table repaired until it is exactly full again. Limiting this way rather
/// than by package-merge costs a fraction of a percent on the ratio, and the
/// repair is a loop over sixteen counters instead of a second algorithm.
fn huffman_lengths(freqs: &[u32], limit: usize) -> Vec<u8> {
    let mut lengths = vec![0u8; freqs.len()];
    let used: Vec<usize> = (0..freqs.len()).filter(|&symbol| freqs[symbol] > 0).collect();
    match used.len() {
        // Nothing to code. For the distance alphabet this is a block of
        // literals only, which is written as one unused code.
        0 => return lengths,
        // One symbol still needs a bit to be read as anything, and a table of
        // a single one-bit code is incomplete -- which every decoder accepts
        // for this case, and is what every other encoder writes.
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

    // Depths, walked iteratively: a tree over 286 symbols is shallow, but a
    // degenerate one is 285 deep and recursion has no business being that.
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

    // Clamping may have made the table more than full, which is not a code at
    // all. Repair: take a code off the longest length, split a shorter one in
    // two to replace it, and repeat until the table is exactly full.
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
    // The other direction never comes up: the tree is a complete code, so its
    // table is exactly full, and clamping a length can only over-fill it.
    debug_assert_eq!(total, full, "the repaired table is not a complete code");

    // Hand the lengths back out, longest to the least frequent symbol, so the
    // repaired table is still the best assignment of it.
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

/// Canonical codes for a set of lengths: shortest first, in symbol order, each
/// code the previous one plus one and shifted when the length grows.
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

/// A block's own code tables, and the description of them that goes in front
/// of the block.
struct DynamicTables {
    literals: Codes,
    distances: Codes,
    /// How many of each alphabet is written, trailing unused symbols dropped.
    literal_count: usize,
    distance_count: usize,
    /// The two length sequences run together and run-length encoded, as
    /// (symbol, extra value) pairs.
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
        // Trailing entries of the permuted order that are zero are not
        // written; four is the fewest the format allows.
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

    /// What the description of these tables costs, so a block can tell whether
    /// they are worth writing at all.
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

/// Run-length encode a sequence of code lengths with symbols 16, 17 and 18,
/// which is how the table describing a block's tables is kept small: a model
/// part's distance alphabet is mostly zeroes, and 138 of them are two symbols.
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
            // The value itself, then repeats of it: symbol 16 copies whatever
            // was written last, so the first one always has to be written out.
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

/// Bits out, least significant first, which is the order DEFLATE packs them
/// in. A Huffman code is the exception: it is written most significant bit
/// first, so [`Bits::push_code`] reverses it.
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
        // Reversed, because a canonical code is written with its most
        // significant bit first while everything else here is written least
        // significant first.
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
