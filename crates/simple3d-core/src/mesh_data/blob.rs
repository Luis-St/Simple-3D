//! The stored form of a mesh, and the colour tags packed alongside it.

use serde::{Deserialize, Serialize};

/// The stored form. `triangles` and `vertices` are for the person reading the
/// file; the loader trusts the arrays and checks them against each other.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MeshBlob {
    pub triangles: usize,
    pub vertices: usize,
    pub positions: String,
    pub indices: String,
    /// Run-length encoded, and absent entirely from a mesh nobody painted --
    /// which is most of them.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub tags: String,
}

/// Per-triangle colour tags as runs of `count:tag`, which for a converted solid
/// is a handful of pairs rather than one number per triangle. An empty string
/// means every triangle is untagged.
pub(crate) fn encode_tags(tags: &[u32], triangles: usize) -> String {
    if tags.iter().all(|&t| t == 0) {
        return String::new();
    }
    let mut out = String::new();
    let mut run_tag = tags.first().copied().unwrap_or(0);
    let mut run = 0usize;
    for index in 0..triangles {
        let tag = tags.get(index).copied().unwrap_or(0);
        if tag == run_tag {
            run += 1;
            continue;
        }
        out.push_str(&format!("{run}:{run_tag} "));
        run_tag = tag;
        run = 1;
    }
    if run > 0 {
        out.push_str(&format!("{run}:{run_tag}"));
    }
    out.trim_end().to_string()
}

pub(crate) fn decode_tags(text: &str, triangles: usize) -> Option<Vec<u32>> {
    if text.is_empty() {
        return Some(vec![0; triangles]);
    }
    let mut out = Vec::with_capacity(triangles);
    for run in text.split_whitespace() {
        let (count, tag) = run.split_once(':')?;
        let count: usize = count.parse().ok()?;
        let tag: u32 = tag.parse().ok()?;
        if out.len() + count > triangles {
            return None;
        }
        out.extend(std::iter::repeat_n(tag, count));
    }
    // A short run list is not an error: it means the rest is untagged.
    out.resize(triangles, 0);
    Some(out)
}
