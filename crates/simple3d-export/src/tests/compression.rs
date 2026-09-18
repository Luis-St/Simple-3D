//! The DEFLATE encoder, and the compressed package it is there for.
//!
//! Every test here decodes with [`simple3d_import::inflate`] rather than with
//! anything of this crate's: an encoder checked against its own decoder proves
//! only that the two agree, and the whole point is that a file this program
//! writes opens in a slicer.

use super::*;
use simple3d_import::inflate::inflate;

/// The kinds of input an encoder gets wrong in different ways: nothing at all,
/// a single byte, a long run, text that is all back-references, data with no
/// structure to find, and data that is all one byte repeated -- the case where
/// a match overlaps itself.
#[test]
pub(crate) fn every_shape_of_input_comes_back_exactly() {
    let mut cases: Vec<Vec<u8>> = vec![
        Vec::new(),
        b"x".to_vec(),
        b"ab".to_vec(),
        vec![b'z'; 300],
        b"<vertex x=\"1\" y=\"2\" z=\"3\"/>\n".repeat(500),
        (0..=255u8).cycle().take(9000).collect(),
    ];
    // Something with no structure to find, from a generator of this test's own
    // so the case is the same on every run.
    let mut state = 0x2545_F491_4F6C_DD1Du64;
    let mut noise = Vec::with_capacity(20000);
    for _ in 0..20000 {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        noise.push(state as u8);
    }
    cases.push(noise);
    // And a model part, which is what this is all for.
    cases.push(model_part(2000));

    for case in &cases {
        let compressed = crate::deflate::deflate(case);
        let back = inflate(&compressed, case.len()).unwrap_or_else(|e| panic!("{} bytes: {e}", case.len()));
        assert_eq!(back.len(), case.len(), "{} bytes came back as {}", case.len(), back.len());
        assert!(back == *case, "{} bytes came back changed", case.len());
    }
}

/// Data with structure in it gets smaller, and data without it does not get
/// bigger: a block that cannot be compressed is written stored, so the worst
/// case is the few bytes of a block header rather than an inflated file.
#[test]
pub(crate) fn compression_helps_where_it_can_and_never_hurts_much() {
    let model = model_part(8000);
    let compressed = crate::deflate::deflate(&model);
    let ratio = model.len() as f64 / compressed.len() as f64;
    assert!(ratio > 3.0, "a model part only compressed {ratio:.1}x");

    let mut state = 0x9E37_79B9_7F4A_7C15u64;
    let noise: Vec<u8> = (0..50000)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state as u8
        })
        .collect();
    let compressed = crate::deflate::deflate(&noise);
    assert!(
        compressed.len() < noise.len() + noise.len() / 100 + 64,
        "incompressible data grew from {} to {}",
        noise.len(),
        compressed.len()
    );
    assert_eq!(inflate(&compressed, noise.len()).unwrap(), noise);
}

/// The option, end to end: a compressed package is smaller, says it is
/// deflated, and holds the same model as a stored one.
#[test]
pub(crate) fn a_three_mf_is_compressed_unless_the_option_says_otherwise() {
    let mesh = many_triangles();
    let compressed_path = temp_dir().join("compressed.3mf");
    let stored_path = temp_dir().join("stored.3mf");
    let mut cb = no_progress();
    write(&compressed_path, &mesh, &Options { format: Format::ThreeMf, ..Default::default() }, &mut cb).unwrap();
    write(&stored_path, &mesh, &Options { format: Format::ThreeMf, compress: false, ..Default::default() }, &mut cb)
        .unwrap();

    let compressed = std::fs::read(&compressed_path).unwrap();
    let stored = std::fs::read(&stored_path).unwrap();
    assert!(compressed.len() * 2 < stored.len(), "compressing {} bytes only got to {}", stored.len(), compressed.len());
    assert_eq!(method_of(&compressed, "3D/3dmodel.model"), Some(8), "the model part was not deflated");
    assert_eq!(method_of(&stored, "3D/3dmodel.model"), Some(0), "the option did not turn compression off");

    // Both are the same model, read back through the importer.
    let from_compressed = simple3d_import::read_bytes(&compressed, None, &mut no_progress()).unwrap();
    let from_stored = simple3d_import::read_bytes(&stored, None, &mut no_progress()).unwrap();
    assert_eq!(from_compressed.triangle_count(), mesh.weld().triangle_count());
    assert_eq!(from_compressed.triangle_count(), from_stored.triangle_count());
    let (lo, hi) = from_compressed.merged().bounds().unwrap();
    let (was_lo, was_hi) = from_stored.merged().bounds().unwrap();
    assert!((lo - was_lo).length() < 1e-9 && (hi - was_hi).length() < 1e-9);
}

/// Exporting the same scene twice produces the same bytes, compressed as well
/// as stored -- the guarantee the fixed timestamp in the zip header exists for,
/// and one an encoder with any randomness in it would break.
#[test]
pub(crate) fn a_compressed_export_is_byte_identical_every_time() {
    let mesh = many_triangles();
    let mut cb = no_progress();
    let first = temp_dir().join("again-1.3mf");
    let second = temp_dir().join("again-2.3mf");
    write(&first, &mesh, &Options { format: Format::ThreeMf, ..Default::default() }, &mut cb).unwrap();
    write(&second, &mesh, &Options { format: Format::ThreeMf, ..Default::default() }, &mut cb).unwrap();
    assert_eq!(std::fs::read(&first).unwrap(), std::fs::read(&second).unwrap());
}

/// A mesh with enough surface to make a model part worth compressing.
fn many_triangles() -> Mesh {
    primitives::ellipsoid_mesh(40.0, 40.0, 40.0, 48)
}

/// XML shaped exactly like the model part of a 3MF, which is the data this
/// encoder exists for. Written here rather than exported and unzipped, so the
/// encoder can be measured on a known number of vertices without a zip reader
/// in the middle.
fn model_part(vertices: usize) -> Vec<u8> {
    let mut out = String::with_capacity(vertices * 90);
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<model unit=\"millimeter\">\n <resources>\n");
    out.push_str("  <object id=\"1\" type=\"model\">\n   <mesh>\n    <vertices>\n");
    // A generator of this test's own, so the case is the same on every run.
    let mut state = 0x1234_5678_9ABC_DEF0u64;
    let mut next = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        (state >> 40) as f64 / 64.0
    };
    for _ in 0..vertices {
        out.push_str(&format!(
            "     <vertex x=\"{}\" y=\"{}\" z=\"{}\"/>\n",
            coord(next()),
            coord(next()),
            coord(next())
        ));
    }
    out.push_str("    </vertices>\n    <triangles>\n");
    for i in 0..vertices * 2 {
        out.push_str(&format!(
            "     <triangle v1=\"{}\" v2=\"{}\" v3=\"{}\"/>\n",
            i % vertices,
            (i * 7) % vertices,
            (i * 13) % vertices
        ));
    }
    out.push_str("    </triangles>\n   </mesh>\n  </object>\n </resources>\n</model>\n");
    out.into_bytes()
}

/// The compression method a zip's central directory records for one entry.
fn method_of(archive: &[u8], name: &str) -> Option<u16> {
    let signature = [0x50u8, 0x4b, 0x01, 0x02];
    let mut at = 0;
    while at + 46 <= archive.len() {
        if archive[at..at + 4] == signature {
            let method = u16::from_le_bytes([archive[at + 10], archive[at + 11]]);
            let name_length = u16::from_le_bytes([archive[at + 28], archive[at + 29]]) as usize;
            let found = String::from_utf8_lossy(&archive[at + 46..at + 46 + name_length]).to_string();
            if found == name {
                return Some(method);
            }
            at += 46 + name_length;
        } else {
            at += 1;
        }
    }
    None
}

/// How long a big model part takes to compress. An export has two minutes
/// before it gives up, and the compression must be a small part of that: this
/// is the check that the encoder is quick enough to be on by default.
#[test]
#[ignore = "a timing check, run with --ignored when the encoder is changed"]
pub(crate) fn a_large_model_part_compresses_quickly() {
    let model = model_part(120_000);
    let started = std::time::Instant::now();
    let compressed = crate::deflate::deflate(&model);
    let elapsed = started.elapsed();
    println!(
        "{:.1} MB -> {:.1} MB ({:.1}x) in {:?}",
        model.len() as f64 / 1e6,
        compressed.len() as f64 / 1e6,
        model.len() as f64 / compressed.len() as f64,
        elapsed
    );
    assert!(elapsed.as_secs_f64() < 20.0, "compressing {} bytes took {elapsed:?}", model.len());
}
