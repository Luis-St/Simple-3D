//! LZ77: literals and back-references found through hash chains.

use super::*;

/// Turn `data` into literals and back-references.
///
/// Every position is entered into the hash chain, including those inside a match; skipping
/// them costs noticeable ratio.
pub(super) fn tokenize(data: &[u8]) -> Vec<Token> {
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
