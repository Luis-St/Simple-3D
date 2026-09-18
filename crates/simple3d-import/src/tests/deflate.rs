//! The DEFLATE decoder, against streams a real compressor produced.
//!
//! The fixtures below were written by zlib at level 9 and are checked against
//! the text they were made from, which is rebuilt here rather than embedded:
//! the two agreeing is the whole point of the test. They cover the two block
//! types a compressor actually emits -- fixed Huffman for a short repetitive
//! run, dynamic Huffman with its own code tables for anything longer -- and
//! both carry back-references, which is where a decoder gets a stream subtly
//! wrong rather than failing outright.

use crate::inflate::inflate;

/// Ten vertex elements, the text `DYNAMIC` was compressed from.
fn vertex_lines() -> String {
    (0..10).map(|i: i32| format!("<vertex x=\"{}.5\" y=\"{}.25\" z=\"-{}\"/>\n", i, i * 3, i * 7)).collect()
}

/// `vertex_lines()` deflated by zlib at level 9: a dynamic-Huffman block, 109
/// bytes standing for 354.
const DYNAMIC: [u8; 109] = [
    0x5d, 0xca, 0x49, 0x0a, 0x80, 0x30, 0x0c, 0x40, 0xd1, 0xbd, 0xa7, 0x28, 0xdd, 0x3b, 0x24, 0x6d, 0x3a, 0x40, 0xf5,
    0x36, 0x5e, 0x40, 0x44, 0xaa, 0xa7, 0x57, 0x21, 0x4d, 0x21, 0xab, 0xff, 0x17, 0xaf, 0x5c, 0xfb, 0x71, 0xee, 0xd5,
    0xd4, 0xd5, 0x2e, 0x13, 0x59, 0x73, 0xff, 0xc5, 0x6f, 0x9e, 0xd5, 0x8e, 0x8b, 0x9d, 0xb7, 0xa1, 0x74, 0x01, 0x2c,
    0x9c, 0x88, 0xa8, 0x04, 0xb2, 0x08, 0x22, 0xc0, 0x2b, 0xe2, 0x98, 0x64, 0x21, 0x08, 0x8a, 0x78, 0x26, 0x80, 0xdd,
    0x24, 0x65, 0xa8, 0x19, 0x12, 0xe3, 0x48, 0x99, 0xd0, 0x4c, 0x12, 0xe3, 0x51, 0x99, 0xc8, 0x06, 0xa1, 0x9b, 0xac,
    0x4c, 0x6a, 0xc6, 0x8b, 0xa1, 0xa0, 0x4c, 0x6e, 0x26, 0x8a, 0x09, 0xee, 0x37, 0x2f,
];

/// Forty identical triangle elements deflated by zlib at level 9: a
/// fixed-Huffman block, 46 bytes standing for 1320 -- which it manages by
/// referring back to what it has already written, over and over.
const FIXED: [u8; 46] = [
    0xb3, 0x29, 0x29, 0xca, 0x4c, 0xcc, 0x4b, 0xcf, 0x49, 0x55, 0x28, 0x33, 0xb4, 0x55, 0x32, 0x50, 0x52, 0x28, 0x33,
    0xb2, 0x55, 0x32, 0x04, 0x52, 0xc6, 0xb6, 0x4a, 0x46, 0x4a, 0xfa, 0x76, 0x5c, 0x36, 0xa3, 0x0a, 0x46, 0x15, 0x8c,
    0x2a, 0x18, 0x55, 0x30, 0xb2, 0x15, 0x00, 0x00,
];

#[test]
pub(crate) fn a_dynamic_huffman_block_is_decoded() {
    let expected = vertex_lines();
    let out = inflate(&DYNAMIC, expected.len()).unwrap();
    assert_eq!(String::from_utf8(out).unwrap(), expected);
}

#[test]
pub(crate) fn a_fixed_huffman_block_with_back_references_is_decoded() {
    let expected = "<triangle v1=\"0\" v2=\"1\" v3=\"2\"/>\n".repeat(40);
    let out = inflate(&FIXED, expected.len()).unwrap();
    assert_eq!(String::from_utf8(out).unwrap(), expected);
}

/// A stored block: no compression at all, which is valid DEFLATE and what the
/// package tests here are built out of.
#[test]
pub(crate) fn a_stored_block_is_copied_out_verbatim() {
    let text = b"<model unit=\"millimeter\"/>";
    let out = inflate(&super::deflate_stored(text), text.len()).unwrap();
    assert_eq!(out, text);
}

/// Several blocks one after another, which is what a compressor emits for
/// anything long: only the last says it is last, and a decoder that stops at
/// the first one silently loses the rest of the file.
#[test]
pub(crate) fn a_stream_of_several_blocks_is_read_to_the_end() {
    let mut stream = super::deflate_stored(b"first ");
    // Clear the "last block" bit of the first block, so the second is reached.
    stream[0] = 0x00;
    stream.extend_from_slice(&super::deflate_stored(b"second"));
    let out = inflate(&stream, 12).unwrap();
    assert_eq!(out, b"first second");
}

/// Refusals, each with its own reason: a stream that ends mid-symbol, a stored
/// block whose length does not match its check field, and a back-reference
/// reaching before the start of the output -- which is the one that would read
/// outside the buffer if it were followed.
#[test]
pub(crate) fn a_broken_stream_is_refused_with_the_reason() {
    let cut = &DYNAMIC[..40];
    assert!(inflate(cut, 354).is_err(), "a truncated stream was accepted");

    let mut bad_check = super::deflate_stored(b"abc");
    bad_check[3] ^= 0xff;
    let why = inflate(&bad_check, 3).unwrap_err();
    assert!(why.contains("check field"), "{why}");

    // A final fixed-Huffman block whose first symbol is the length 3 (code
    // 0000001) followed by distance 1 (00000), with nothing written yet for it
    // to refer back to.
    let backwards = [0x03, 0x02, 0x00, 0x00];
    let why = inflate(&backwards, 16).unwrap_err();
    assert!(why.contains("past the start"), "{why}");
}
