//! Which colour the boolean preview draws each shape's faces in (issue 114).

use crate::app::{CSG_DIFFERENCE, CSG_INTERSECTION};

/// Per shape, the colour its faces take instead of their own. A cutter's faces left in the result
/// are the walls of the cut, the inside of what it cuts, so they take the colour of that shape's
/// first body, as the evaluation does (`csg_bsp::inherit`); so do a section's half-space shapes
/// from `half`, whose faces are the cap. `None` keeps the shape's own paint.
pub(super) fn inherited(program: &[i32], firsts: &[u32], half: Option<usize>) -> Vec<Option<[f32; 4]>> {
    let mut source: Vec<Option<usize>> = vec![None; firsts.len()];
    let mut stack: Vec<Vec<usize>> = Vec::new();
    for &code in program {
        if code >= 0 {
            stack.push(vec![code as usize]);
            continue;
        }
        let (Some(right), Some(mut left)) = (stack.pop(), stack.pop()) else { break };
        let halves = half.is_some_and(|first| right.iter().all(|&shape| shape >= first));
        if code == CSG_DIFFERENCE || (code == CSG_INTERSECTION && halves) {
            for &shape in &right {
                source[shape] = Some(left[0]);
            }
        }
        left.extend(right);
        stack.push(left);
    }
    (0..firsts.len())
        .map(|shape| {
            let mut at = source.get(shape).copied().flatten()?;
            // Sources point into earlier operands, so this ends; bounded all the same.
            for _ in 0..firsts.len() {
                match source.get(at).copied().flatten() {
                    Some(next) => at = next,
                    None => break,
                }
            }
            let [r, g, b] = simple3d_geom::tag_colour(*firsts.get(at)?)?;
            Some([r as f32, g as f32, b as f32, 255.0])
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cutters_walls_take_the_colour_of_what_it_cuts() {
        let red = simple3d_geom::colour_tag([200, 0, 0]);
        let blue = simple3d_geom::colour_tag([0, 0, 200]);
        // (red - blue) - plain: both cutters show red walls; the base keeps its own paint.
        let program = [0, 1, CSG_DIFFERENCE, 2, CSG_DIFFERENCE];
        let colours = inherited(&program, &[red, blue, 0], None);
        assert_eq!(colours[0], None);
        assert_eq!(colours[1], Some([200.0, 0.0, 0.0, 255.0]));
        assert_eq!(colours[2], Some([200.0, 0.0, 0.0, 255.0]));
        // A union's operands keep their own colours, and an unpainted base leaves the cutter as it is.
        assert_eq!(inherited(&[0, 1, crate::app::CSG_UNION], &[red, blue], None), vec![None, None]);
        assert_eq!(inherited(&[0, 1, CSG_DIFFERENCE], &[0, blue], None), vec![None, None]);
        // A section's half-space is the cap, in the colour of what it cuts; another shape intersected is not.
        assert_eq!(inherited(&[0, 1, CSG_INTERSECTION], &[red, 0], Some(1))[1], Some([200.0, 0.0, 0.0, 255.0]));
        assert_eq!(inherited(&[0, 1, CSG_INTERSECTION], &[red, 0], None)[1], None);
    }
}
