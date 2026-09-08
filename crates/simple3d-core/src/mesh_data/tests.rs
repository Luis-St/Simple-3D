use super::*;
use simple3d_geom::primitives;

#[test]
fn a_mesh_survives_the_file_it_is_written_to() {
    let mut mesh = primitives::ellipsoid_mesh(40.0, 30.0, 20.0, 24);
    mesh.set_tag(simple3d_geom::colour_tag([0x2E, 0x9A, 0xFF]));
    let data = MeshData::new(mesh);
    let blob = data.to_blob();
    let back = MeshData::from_blob(&blob).expect("it should read back");
    assert_eq!(back.mesh.indices, data.mesh.indices);
    assert_eq!(back.mesh.tags, data.mesh.tags);
    assert_eq!(back.mesh.positions.len(), data.mesh.positions.len());
    for (a, b) in back.mesh.positions.iter().zip(&data.mesh.positions) {
        // Stored as f32: exact to the last place a millimetre-scale model
        // has, which is what the doc comment claims.
        assert!((*a - *b).length() < 1e-3, "{a:?} came back as {b:?}");
    }
}

#[test]
fn several_colours_survive_as_runs() {
    let mut mesh = primitives::box_mesh(10.0, 10.0, 10.0);
    mesh.set_tag(7);
    let half = mesh.indices.len() / 2;
    for tag in mesh.tags.iter_mut().take(half) {
        *tag = 9;
    }
    let data = MeshData::new(mesh.clone());
    let blob = data.to_blob();
    assert!(blob.tags.contains(':'), "the runs were not written: {:?}", blob.tags);
    let back = MeshData::from_blob(&blob).unwrap();
    assert_eq!(back.mesh.tags, data.mesh.tags);
}

#[test]
fn an_unpainted_mesh_writes_no_tags_at_all() {
    // So a mesh body diffs as its geometry and nothing else.
    let blob = MeshData::new(primitives::box_mesh(5.0, 5.0, 5.0)).to_blob();
    assert!(blob.tags.is_empty());
    assert_eq!(MeshData::from_blob(&blob).unwrap().mesh.tags.len(), blob.triangles);
}

#[test]
fn a_damaged_blob_is_refused_rather_than_half_read() {
    let good = MeshData::new(primitives::box_mesh(10.0, 10.0, 10.0)).to_blob();

    let mut truncated = good.clone();
    truncated.positions.truncate(good.positions.len() - 6);
    assert!(MeshData::from_blob(&truncated).is_none(), "a truncated vertex array was accepted");

    let mut wrong = good.clone();
    // An index array of a length that is not a whole number of triangles.
    wrong.indices.truncate(4);
    assert!(MeshData::from_blob(&wrong).is_none());

    let mut past_the_end = good.clone();
    past_the_end.positions = encode(&[0u8; 12]);
    assert!(MeshData::from_blob(&past_the_end).is_none(), "an index past the last vertex was accepted");

    let mut nonsense = good;
    nonsense.positions = "not base64 at all !!!".into();
    assert!(MeshData::from_blob(&nonsense).is_none());
}

#[test]
fn base64_round_trips_every_length() {
    for len in 0..40usize {
        let bytes: Vec<u8> = (0..len).map(|i| (i * 37 % 251) as u8).collect();
        assert_eq!(decode(&encode(&bytes)).unwrap(), bytes, "length {len}");
    }
    // And it is the standard encoding, not a private one.
    assert_eq!(encode(b"Man"), "TWFu");
    assert_eq!(encode(b"Ma"), "TWE=");
    assert_eq!(encode(b"M"), "TQ==");
}

#[test]
fn a_stored_mesh_is_written_on_a_handful_of_lines() {
    // The reason for the encoding: a converted tile must not turn the
    // project file into something no editor will open.
    let mesh = primitives::ellipsoid_mesh(40.0, 40.0, 40.0, 64);
    assert!(mesh.triangle_count() > 4000, "the test needs a big mesh");
    let text = serde_json::to_string_pretty(&MeshData::new(mesh).to_blob()).unwrap();
    assert!(text.lines().count() < 10, "{} lines", text.lines().count());
}
