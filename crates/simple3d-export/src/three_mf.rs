//! 3MF: the one format here that carries several bodies and their colours.

use super::*;
use simple3d_geom::{tag_colour, Mesh};

/// Trim a coordinate to a fixed number of decimals without floating-point
/// noise, matching what the property editor shows the user.
pub(crate) fn coord(v: f64) -> String {
    let mut s = format!("{v:.6}");
    if s.contains('.') {
        while s.ends_with('0') {
            s.pop();
        }
        if s.ends_with('.') {
            s.pop();
        }
    }
    if s == "-0" {
        s = "0".into();
    }
    s
}

/// The distinct colours the faces are painted, in the order they first appear,
/// together with each triangle's index into that list -- one list per mesh, in
/// the order the meshes were given. Index 0 is always the unpainted default, so
/// an unpainted model yields a list of one and nothing downstream has to
/// special-case it.
///
/// The table spans every mesh because the file has one colour group for the
/// whole model: two objects painted the same colour name the same entry.
pub(crate) fn colour_table(meshes: &[&Mesh]) -> (Vec<[u8; 3]>, Vec<Vec<usize>>) {
    // The colour an unpainted surface is given in the file. 3MF has no "no
    // colour" for a face inside a coloured object, so this is the neutral the
    // viewport would have drawn.
    const DEFAULT: [u8; 3] = [0x9A, 0xA4, 0xB2];
    let mut colours = vec![DEFAULT];
    let mut per_mesh = Vec::with_capacity(meshes.len());
    for mesh in meshes {
        let mut per_triangle = Vec::with_capacity(mesh.indices.len());
        for i in 0..mesh.indices.len() {
            let index = match tag_colour(mesh.tag(i)) {
                None => 0,
                Some(rgb) => colours.iter().position(|c| *c == rgb).unwrap_or_else(|| {
                    colours.push(rgb);
                    colours.len() - 1
                }),
            };
            per_triangle.push(index);
        }
        per_mesh.push(per_triangle);
    }
    (colours, per_mesh)
}

/// XML text escaping, for the one place a name the user typed reaches a file.
pub(crate) fn escape_xml(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            // A control character is not valid XML 1.0 text at all; the file
            // has to stay parseable whatever a node was called.
            c if (c as u32) < 0x20 && c != '\t' && c != '\n' && c != '\r' => out.push(' '),
            c => out.push(c),
        }
    }
    out
}

/// The 3MF document for `parts`: one `<object>` each, one `<item>` each in the
/// build, and one colour group for the model if anything in it is painted.
///
/// Object ids run 1..=n so a single-part model is written exactly as it always
/// was, and the colour group takes the id after the last object -- it is
/// emitted first all the same, because a resource has to be declared before the
/// object that names it.
pub(crate) fn three_mf(parts: &[Part<'_>], options: &Options, progress: Progress<'_>) -> Result<Vec<u8>, ExportError> {
    let meshes: Vec<&Mesh> = parts.iter().map(|part| part.mesh).collect();
    let (colours, triangle_colour) = colour_table(&meshes);
    // Only a painted model carries the materials extension: an unpainted one
    // is written exactly as it was before colours existed, so nothing that
    // reads plain 3MF has to cope with a namespace it does not need.
    let painted = colours.len() > 1;
    let colour_group_id = parts.len() + 1;
    let total_vertices: usize = meshes.iter().map(|m| m.positions.len()).sum();
    let total_triangles: usize = meshes.iter().map(|m| m.indices.len()).sum();

    let mut model = String::with_capacity(total_vertices * 48 + total_triangles * 40);
    model.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    model.push_str(&format!(
        "<model unit=\"{}\" xml:lang=\"en-US\" \
         xmlns=\"http://schemas.microsoft.com/3dmanufacturing/core/2015/02\"{}>\n",
        options.unit.as_str(),
        if painted { " xmlns:m=\"http://schemas.microsoft.com/3dmanufacturing/material/2015/02\"" } else { "" }
    ));
    model.push_str(" <resources>\n");
    if painted {
        // One colour group holding every colour in the model; each triangle
        // then names its own entry. Not declared as a required extension: a
        // reader that ignores colour still gets the whole solid.
        model.push_str(&format!("  <m:colorgroup id=\"{colour_group_id}\">\n"));
        for rgb in &colours {
            model.push_str(&format!("   <m:color color=\"#{:02X}{:02X}{:02X}\"/>\n", rgb[0], rgb[1], rgb[2]));
        }
        model.push_str("  </m:colorgroup>\n");
    }

    // Progress across every part together, so the bar means the same thing
    // whether one object is being written or twenty.
    let mut vertices_done = 0usize;
    let mut triangles_done = 0usize;
    for (index, part) in parts.iter().enumerate() {
        let mesh = part.mesh;
        let name = if part.name.is_empty() { String::new() } else { format!(" name=\"{}\"", escape_xml(part.name)) };
        model.push_str(&format!(
            "  <object id=\"{}\" type=\"model\"{name}{}>\n   <mesh>\n    <vertices>\n",
            index + 1,
            if painted { format!(" pid=\"{colour_group_id}\" pindex=\"0\"") } else { String::new() }
        ));
        for (i, p) in mesh.positions.iter().enumerate() {
            model.push_str(&format!("     <vertex x=\"{}\" y=\"{}\" z=\"{}\"/>\n", coord(p.x), coord(p.y), coord(p.z)));
            if i % 4096 == 0 && !progress(0.2 + 0.4 * ((vertices_done + i) as f32 / total_vertices.max(1) as f32)) {
                return Err(ExportError::Cancelled);
            }
        }
        vertices_done += mesh.positions.len();
        model.push_str("    </vertices>\n    <triangles>\n");
        for (i, t) in mesh.indices.iter().enumerate() {
            let paint = if painted {
                // One index for the whole triangle: p1 alone means a flat face,
                // which is what a painted body has.
                format!(" p1=\"{}\"", triangle_colour[index][i])
            } else {
                String::new()
            };
            model.push_str(&format!("     <triangle v1=\"{}\" v2=\"{}\" v3=\"{}\"{paint}/>\n", t[0], t[1], t[2]));
            if i % 4096 == 0 && !progress(0.6 + 0.3 * ((triangles_done + i) as f32 / total_triangles.max(1) as f32)) {
                return Err(ExportError::Cancelled);
            }
        }
        triangles_done += mesh.indices.len();
        model.push_str("    </triangles>\n   </mesh>\n  </object>\n");
    }
    model.push_str(" </resources>\n <build>\n");
    for index in 0..parts.len() {
        model.push_str(&format!("  <item objectid=\"{}\"/>\n", index + 1));
    }
    model.push_str(" </build>\n</model>\n");

    const CONTENT_TYPES: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
        <Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\">\
        <Default Extension=\"rels\" \
        ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/>\
        <Default Extension=\"model\" \
        ContentType=\"application/vnd.ms-package.3dmanufacturing-3dmodel+xml\"/>\
        </Types>\n";
    const RELS: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
        <Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\
        <Relationship Id=\"rel0\" Target=\"/3D/3dmodel.model\" \
        Type=\"http://schemas.microsoft.com/3dmanufacturing/2013/01/3dmodel\"/>\
        </Relationships>\n";

    let mut archive = zip::ZipWriter::new();
    archive.add("[Content_Types].xml", CONTENT_TYPES.as_bytes());
    archive.add("_rels/.rels", RELS.as_bytes());
    archive.add("3D/3dmodel.model", model.as_bytes());
    Ok(archive.finish())
}
