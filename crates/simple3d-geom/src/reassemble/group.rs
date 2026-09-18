//! Which parts belong together.

use super::*;

/// Gather the parts that touch into groups, as indices into `parts`.
///
/// Touching is read off the boxes rather than off the triangles, and
/// deliberately: what the grouping is for is saying that a pin and the plate it
/// stands in were modelled as one thing, and a box is the scale that question
/// is asked at. Measuring real contact would be a boolean per pair -- hundreds
/// of them, each of them the slowest thing in the crate -- to answer a question
/// about intent that the extra precision would not answer any better.
///
/// Parts that touch a common third part end up in one group, because they do
/// stand together even where they do not touch each other: a shaft through two
/// brackets is one assembly.
///
/// With `together` off every part is a group of its own, which is to say no
/// groups at all.
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
    // Kept in the order the parts are in, so the groups come out biggest first
    // exactly as the parts do.
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

/// Whether two boxes meet, with enough slack that solids merely resting against
/// each other count -- which is the ordinary way an assembly is modelled, and
/// the case the grouping is entirely for.
fn touching(a: (Vec3, Vec3), b: (Vec3, Vec3)) -> bool {
    let ((alo, ahi), (blo, bhi)) = (a, b);
    alo.x - SLACK <= bhi.x
        && blo.x - SLACK <= ahi.x
        && alo.y - SLACK <= bhi.y
        && blo.y - SLACK <= ahi.y
        && alo.z - SLACK <= bhi.z
        && blo.z - SLACK <= ahi.z
}

/// How far apart, in millimetres, two bodies may stand and still be called
/// touching. A hundredth of a millimetre: under the resolution of every file
/// format this reads, so surfaces that were meant to be in contact still are
/// after the round trip, and far under any gap anybody models on purpose.
const SLACK: f64 = 0.01;
