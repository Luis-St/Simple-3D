//! The drawn curves as a planar graph: their polylines split wherever they cross or touch, with
//! nearby ends merged into one vertex.

use crate::curve::{CurveId, Shape};
use crate::sample::{arc_angles, sample_curve};
use crate::sketch::Sketch;
use crate::{cross, distance, dot, sub};
use std::collections::HashMap;

/// Vertices this close are one, in millimetres: looser than the solver's tolerance, so ends the
/// solver joined always meet.
pub(crate) const MERGE: f64 = 1e-5;

/// A stretch of one curve between two vertices.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Edge {
    pub u: usize,
    pub v: usize,
    pub curve: CurveId,
    /// Whether going from `u` to `v` follows the curve's own direction.
    pub forward: bool,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Graph {
    pub pos: Vec<[f64; 2]>,
    pub edges: Vec<Edge>,
}

struct Segment {
    a: [f64; 2],
    b: [f64; 2],
    curve: CurveId,
    /// Where it is split, as fractions along it.
    cuts: Vec<f64>,
}

/// Whether point `p` lies on curve `shape` closely enough to be a vertex of it.
fn lies_on(sketch: &Sketch, shape: &Shape, p: [f64; 2]) -> bool {
    match *shape {
        Shape::Line { a, b } => {
            let (a, b) = (sketch.at(a), sketch.at(b));
            let d = sub(b, a);
            let t = dot(sub(p, a), d) / dot(d, d).max(1e-24);
            t > 0.0 && t < 1.0 && (cross(d, sub(p, a)) / distance(a, b).max(1e-12)).abs() < crate::EPS
        }
        Shape::Circle { center, radius } => (distance(p, sketch.at(center)) - radius.abs()).abs() < crate::EPS,
        Shape::Arc { center, start, end } => {
            let (c, s, e) = (sketch.at(center), sketch.at(start), sketch.at(end));
            let (a0, sweep) = arc_angles(c, s, e);
            let along = ((p[1] - c[1]).atan2(p[0] - c[0]) - a0).rem_euclid(std::f64::consts::TAU);
            (distance(p, c) - distance(s, c)).abs() < crate::EPS && along < sweep
        }
        _ => false,
    }
}

/// The polyline of every curve that can bound a region, with the sketch points lying on it put in
/// as vertices: a line ending on a circle must meet the circle's polyline exactly, not the chord
/// passing a hair inside it.
fn polylines(sketch: &Sketch, segments: u32) -> Vec<(CurveId, Vec<[f64; 2]>)> {
    let mut out = Vec::new();
    for curve in sketch.curves.iter().filter(|c| !c.construction) {
        let samples = sample_curve(sketch, curve, segments);
        let mut points = samples.points;
        if points.len() < 2 {
            continue;
        }
        if samples.closed {
            points.push(points[0]);
        }
        for p in sketch.points.iter().map(|p| p.at) {
            if !lies_on(sketch, &curve.shape, p) || points.iter().any(|q| distance(*q, p) < MERGE) {
                continue;
            }
            let nearest = (0..points.len() - 1).min_by(|&i, &j| {
                let d = |k: usize| segment_distance(p, points[k], points[k + 1]);
                d(i).total_cmp(&d(j))
            });
            if let Some(i) = nearest {
                points.insert(i + 1, p);
            }
        }
        out.push((curve.id, points));
    }
    out
}

fn segment_distance(p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let d = sub(b, a);
    let t = (dot(sub(p, a), d) / dot(d, d).max(1e-24)).clamp(0.0, 1.0);
    distance(p, crate::add(a, crate::scale(d, t)))
}

/// Split segments `i` and `j` where they cross, touch, or run along each other.
fn intersect(segments: &mut [Segment], i: usize, j: usize) {
    let (p, r) = (segments[i].a, sub(segments[i].b, segments[i].a));
    let (q, s) = (segments[j].a, sub(segments[j].b, segments[j].a));
    let (lr, ls) = (crate::length(r).max(1e-12), crate::length(s).max(1e-12));
    let denom = cross(r, s);
    let (er, es) = (MERGE / lr, MERGE / ls);
    if denom.abs() > 1e-12 * lr * ls {
        let t = cross(sub(q, p), s) / denom;
        let u = cross(sub(q, p), r) / denom;
        if t > -er && t < 1.0 + er && u > -es && u < 1.0 + es {
            segments[i].cuts.push(t.clamp(0.0, 1.0));
            segments[j].cuts.push(u.clamp(0.0, 1.0));
        }
        return;
    }
    // Parallel: split each at the other's ends where they lie along one line.
    if (cross(r, sub(q, p)) / lr).abs() > MERGE {
        return;
    }
    for end in [q, segments[j].b] {
        let t = dot(sub(end, p), r) / (lr * lr);
        if t > 0.0 && t < 1.0 {
            segments[i].cuts.push(t);
        }
    }
    for end in [p, segments[i].b] {
        let u = dot(sub(end, q), s) / (ls * ls);
        if u > 0.0 && u < 1.0 {
            segments[j].cuts.push(u);
        }
    }
}

/// The planar graph of `sketch`'s non-construction curves.
pub(crate) fn build(sketch: &Sketch, segments: u32) -> Graph {
    let mut list: Vec<Segment> = Vec::new();
    for (curve, points) in polylines(sketch, segments) {
        for pair in points.windows(2) {
            if distance(pair[0], pair[1]) > MERGE {
                list.push(Segment { a: pair[0], b: pair[1], curve, cuts: vec![0.0, 1.0] });
            }
        }
    }
    // Swept along X, so only segments whose X ranges overlap are compared.
    let lo = |s: &Segment| s.a[0].min(s.b[0]);
    let hi = |s: &Segment| s.a[0].max(s.b[0]);
    let mut order: Vec<usize> = (0..list.len()).collect();
    order.sort_by(|&i, &j| lo(&list[i]).total_cmp(&lo(&list[j])));
    for (k, &i) in order.iter().enumerate() {
        let (ymin, ymax) = (list[i].a[1].min(list[i].b[1]), list[i].a[1].max(list[i].b[1]));
        for &j in &order[k + 1..] {
            if lo(&list[j]) > hi(&list[i]) + MERGE {
                break;
            }
            if list[j].a[1].min(list[j].b[1]) > ymax + MERGE || list[j].a[1].max(list[j].b[1]) < ymin - MERGE {
                continue;
            }
            intersect(&mut list, i, j);
        }
    }
    let mut graph = Graph::default();
    let mut grid: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
    let mut vertex = |p: [f64; 2], graph: &mut Graph| -> usize {
        let cell = |v: f64| (v / (4.0 * MERGE)).floor() as i64;
        let (cx, cy) = (cell(p[0]), cell(p[1]));
        for dx in -1..=1 {
            for dy in -1..=1 {
                for &k in grid.get(&(cx + dx, cy + dy)).into_iter().flatten() {
                    if distance(graph.pos[k], p) < MERGE {
                        return k;
                    }
                }
            }
        }
        graph.pos.push(p);
        grid.entry((cx, cy)).or_default().push(graph.pos.len() - 1);
        graph.pos.len() - 1
    };
    let mut seen: HashMap<(usize, usize), ()> = HashMap::new();
    for segment in &mut list {
        segment.cuts.sort_by(f64::total_cmp);
        let d = sub(segment.b, segment.a);
        let ids: Vec<usize> = segment
            .cuts
            .iter()
            .map(|&t| vertex(crate::add(segment.a, crate::scale(d, t)), &mut graph))
            .collect();
        for pair in ids.windows(2) {
            let (u, v) = (pair[0], pair[1]);
            if u == v || seen.insert((u.min(v), u.max(v)), ()).is_some() {
                continue;
            }
            graph.edges.push(Edge { u, v, curve: segment.curve, forward: true });
        }
    }
    graph
}
