//! The dense linear algebra the solver needs: a weighted least-change Newton step, and elimination
//! that tells which rows add nothing and which unknowns the rows pin down. Sketches are small enough
//! that dense matrices cost nothing worth a sparse library.

/// A dense matrix, row by row.
pub(crate) type Matrix = Vec<Vec<f64>>;

/// The step `dx` with `J dx = -r` that changes the unknowns least, each change weighed by
/// `weights` (a heavy unknown is moved reluctantly): `dx = -W^-1 J^T (J W^-1 J^T + mu I)^-1 r`. The
/// small `mu` lets rows that repeat each other through. `None` if the system cannot be factored.
pub(crate) fn least_change_step(j: &Matrix, r: &[f64], weights: &[f64]) -> Option<Vec<f64>> {
    let m = r.len();
    let n = weights.len();
    let mut a = vec![vec![0.0; m]; m];
    let mut largest = 0.0_f64;
    for row in 0..m {
        for col in row..m {
            let value: f64 = (0..n).map(|k| j[row][k] * j[col][k] / weights[k]).sum();
            a[row][col] = value;
            a[col][row] = value;
        }
        largest = largest.max(a[row][row]);
    }
    let mu = 1e-12 * largest.max(1e-12);
    for (row, line) in a.iter_mut().enumerate() {
        line[row] += mu;
    }
    let y = cholesky_solve(a, r)?;
    Some((0..n).map(|k| -(0..m).map(|row| j[row][k] * y[row]).sum::<f64>() / weights[k]).collect())
}

/// Solve `a y = b` for a symmetric positive definite `a`, by Cholesky.
fn cholesky_solve(mut a: Matrix, b: &[f64]) -> Option<Vec<f64>> {
    let m = b.len();
    for i in 0..m {
        for jx in 0..=i {
            let mut sum = a[i][jx];
            for k in 0..jx {
                sum -= a[i][k] * a[jx][k];
            }
            if i == jx {
                if sum <= 0.0 || !sum.is_finite() {
                    return None;
                }
                a[i][i] = sum.sqrt();
            } else {
                a[i][jx] = sum / a[jx][jx];
            }
        }
    }
    let mut y = b.to_vec();
    for i in 0..m {
        for k in 0..i {
            y[i] -= a[i][k] * y[k];
        }
        y[i] /= a[i][i];
    }
    for i in (0..m).rev() {
        for k in i + 1..m {
            y[i] -= a[k][i] * y[k];
        }
        y[i] /= a[i][i];
    }
    Some(y)
}

/// Rows reduced one after another, each against the ones kept before it: what the rows span, and
/// for every row that added nothing, which earlier rows it is made of.
pub(crate) struct Elimination {
    /// The kept rows, reduced, each with its pivot column.
    basis: Vec<(Vec<f64>, usize)>,
    /// Each kept row as a combination of the original rows.
    combos: Vec<Vec<f64>>,
    /// Every row that added nothing, with the original rows it is a combination of (itself included).
    pub redundant: Vec<(usize, Vec<usize>)>,
}

impl Elimination {
    pub fn of(j: &Matrix) -> Elimination {
        let m = j.len();
        let mut elimination = Elimination { basis: Vec::new(), combos: Vec::new(), redundant: Vec::new() };
        for (index, row) in j.iter().enumerate() {
            let scale = row.iter().fold(0.0_f64, |most, v| most.max(v.abs()));
            let mut combo = vec![0.0; m];
            combo[index] = 1.0;
            let mut reduced = row.clone();
            elimination.reduce(&mut reduced, Some(&mut combo));
            let (pivot, size) =
                reduced.iter().enumerate().fold((0, 0.0_f64), |best, (k, v)| if v.abs() > best.1 { (k, v.abs()) } else { best });
            if size <= 1e-9 * scale.max(1e-12) || scale == 0.0 {
                let made_of = (0..m).filter(|&k| combo[k].abs() > 1e-9).collect();
                elimination.redundant.push((index, made_of));
            } else {
                elimination.basis.push((reduced, pivot));
                elimination.combos.push(combo);
            }
        }
        elimination
    }

    /// Reduce `row` against every kept row in order, tracking the combination when asked.
    fn reduce(&self, row: &mut [f64], mut combo: Option<&mut Vec<f64>>) {
        for (k, (basis, pivot)) in self.basis.iter().enumerate() {
            let factor = row[*pivot] / basis[*pivot];
            if factor == 0.0 {
                continue;
            }
            for (value, b) in row.iter_mut().zip(basis) {
                *value -= factor * b;
            }
            if let Some(combo) = combo.as_deref_mut() {
                for (value, c) in combo.iter_mut().zip(&self.combos[k]) {
                    *value -= factor * c;
                }
            }
        }
    }

    /// How many independent rows there are.
    pub fn rank(&self) -> usize {
        self.basis.len()
    }

    /// Whether unknown `k` is pinned down by the rows: whether it lies in what they span.
    pub fn pins(&self, k: usize, n: usize) -> bool {
        let mut unit = vec![0.0; n];
        unit[k] = 1.0;
        self.reduce(&mut unit, None);
        unit.iter().all(|v| v.abs() < 1e-7)
    }
}
