//! Which parts belong together.

use super::*;

/// Gather touching parts into groups, as indices into `parts`. Contact is read off boxes, the scale
/// of the question (was this modelled as one thing?), rather than a boolean per pair. Touching is
/// transitive, so a shaft through two brackets is one group. With `together` off, each part stands alone.
pub(super) fn gather(parts: &[Part], together: bool) -> Vec<Vec<usize>> {
    if !together {
        return (0..parts.len()).map(|i| vec![i]).collect();
    }
    let mut owner: Vec<usize> = (0..parts.len()).collect();
    fn root(owner: &mut [usize], mut i: usize) -> usize {
        while owner[i] != i {
            owner[i] = owner[owner[i]];
            i = owner[i];
        }
        i
    }
    for a in 0..parts.len() {
        for b in a + 1..parts.len() {
            if !touching(parts[a].bounds, parts[b].bounds) {
                continue;
            }
            let (ra, rb) = (root(&mut owner, a), root(&mut owner, b));
            if ra != rb {
                owner[ra] = rb;
            }
        }
    }
    // In part order, so groups come out biggest first like the parts.
    let mut groups: Vec<Vec<usize>> = Vec::new();
    let mut place: Vec<Option<usize>> = vec![None; parts.len()];
    for i in 0..parts.len() {
        let owned = root(&mut owner, i);
        let at = *place[owned].get_or_insert_with(|| {
            groups.push(Vec::new());
            groups.len() - 1
        });
        groups[at].push(i);
    }
    groups
}

/// Whether two boxes meet, with slack so resting solids count.
fn touching(a: (Vec3, Vec3), b: (Vec3, Vec3)) -> bool {
    let ((alo, ahi), (blo, bhi)) = (a, b);
    alo.x - SLACK <= bhi.x
        && blo.x - SLACK <= ahi.x
        && alo.y - SLACK <= bhi.y
        && blo.y - SLACK <= ahi.y
        && alo.z - SLACK <= bhi.z
        && blo.z - SLACK <= ahi.z
}

/// How far apart touching bodies may be: 0.01 mm, under every format's resolution and any
/// intentional gap.
const SLACK: f64 = 0.01;
