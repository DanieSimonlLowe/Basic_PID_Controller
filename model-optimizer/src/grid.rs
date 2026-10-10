//! The coarse search grid over b, and detection of local-minimum brackets.

use crate::error::FitError;

/// Maximum number of grid points we are willing to evaluate.
const MAX_GRID_POINTS: f64 = 5.0e7;

/// A candidate region for a local minimum of L*(b).
///
/// Interior minimum:   left < middle < right, loss(middle) is below both.
/// Boundary minimum:   middle equals `left` (lower bound of the search) or
///                     `right` (upper bound of the search).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Candidate {
    pub left: f64,
    pub middle: f64,
    pub right: f64,
}

/// Build the grid `b_min, b_min + step, ...` ending exactly at `b_max`.
/// If `(b_max - b_min)` is not a multiple of `step`, the last interval is
/// shorter.
pub fn build_grid(b_min: f64, b_max: f64, step: f64) -> Result<Vec<f64>, FitError> {
    if !(b_min.is_finite() && b_max.is_finite() && step.is_finite()) {
        return Err(FitError::Config(
            "b-min, b-max and b-step must be finite".into(),
        ));
    }
    if b_max <= b_min {
        return Err(FitError::Config("b-max must be > b-min".into()));
    }
    if step <= 0.0 {
        return Err(FitError::Config("b-step must be > 0".into()));
    }
    if (b_max - b_min) / step > MAX_GRID_POINTS {
        return Err(FitError::Config("grid too large; increase b-step".into()));
    }

    let mut grid = Vec::new();
    let mut k = 0usize;
    loop {
        let b = b_min + k as f64 * step;
        // Stop once we reach b_max (ignoring rounding), then add b_max itself.
        if b >= b_max - 1e-9 * step {
            break;
        }
        grid.push(f64::powf(10.0, b));
        k += 1;
    }
    grid.push(f64::powf(10.0, b_max));

    if grid.len() < 3 {
        return Err(FitError::Config(
            "grid needs at least 3 points; decrease b-step".into(),
        ));
    }
    Ok(grid)
}

/// Find brackets that may contain a local minimum of `loss` sampled on `grid`.
///
/// * every interior strict local minimum  (loss[k-1] > loss[k] < loss[k+1]);
/// * the lower boundary if loss[0] < loss[1];
/// * the upper boundary if loss[n-1] < loss[n-2];
/// * if none of the above (e.g. a flat loss), a bracket around the grid minimum.
pub fn find_candidates(grid: &[f64], loss: &[f64]) -> Result<Vec<Candidate>, FitError> {
    let n = grid.len();
    if n != loss.len() || n < 3 {
        return Err(FitError::Config(
            "need matching grid/loss arrays with at least 3 points".into(),
        ));
    }
    let mut out = Vec::new();

    if loss[0] < loss[1] {
        out.push(Candidate {
            left: grid[0],
            middle: grid[0],
            right: grid[1],
        });
    }
    for k in 1..n - 1 {
        if loss[k - 1] > loss[k] && loss[k + 1] > loss[k] {
            out.push(Candidate {
                left: grid[k - 1],
                middle: grid[k],
                right: grid[k + 1],
            });
        }
    }
    if loss[n - 1] < loss[n - 2] {
        out.push(Candidate {
            left: grid[n - 2],
            middle: grid[n - 1],
            right: grid[n - 1],
        });
    }

    if out.is_empty() {
        // Ties/plateaus: fall back to the first grid minimum and its neighbours.
        let k = loss
            .iter()
            .enumerate()
            .min_by(|x, y| x.1.total_cmp(y.1))
            .map_or(0, |(i, _)| i);
        out.push(Candidate {
            left: grid[k.saturating_sub(1)],
            middle: grid[k],
            right: grid[(k + 1).min(n - 1)],
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pts(n: usize) -> Vec<f64> {
        (1..=n).map(|k| k as f64).collect()
    }

    // ---- build_grid ----

    #[test]
    fn exact_multiple_of_step() {
        let g = build_grid(1.0, 2.0, 0.25).unwrap();
        assert_eq!(g, vec![1.0, 1.25, 1.5, 1.75, 2.0]);
    }

    #[test]
    fn non_multiple_ends_exactly_at_b_max() {
        let g = build_grid(1.0, 2.0, 0.3).unwrap();
        assert_eq!(g.len(), 5);
        assert!((g[3] - 1.9).abs() < 1e-12);
        assert_eq!(*g.last().unwrap(), 2.0);
    }

    #[test]
    fn typical_cli_example() {
        let g = build_grid(0.001, 10.0, 0.01).unwrap();
        assert_eq!(g.len(), 1001);
        assert_eq!(g[0], 0.001);
        assert_eq!(*g.last().unwrap(), 10.0);
        assert!(g[g.len() - 2] < 10.0);
    }

    #[test]
    fn grid_is_strictly_increasing() {
        for (lo, hi, step) in [(0.1, 3.0, 0.07), (0.5, 0.9, 0.1), (1e-3, 1.0, 1e-3)] {
            let g = build_grid(lo, hi, step).unwrap();
            assert!(g.windows(2).all(|w| w[1] > w[0]), "{lo} {hi} {step}");
            assert_eq!(g[0], lo);
            assert_eq!(*g.last().unwrap(), hi);
        }
    }

    #[test]
    fn rejects_invalid_configuration() {
        let bad = [
            (0.0, 1.0, 0.1),           // b_min == 0
            (-1.0, 1.0, 0.1),          // b_min < 0
            (1.0, 1.0, 0.1),           // b_max == b_min
            (2.0, 1.0, 0.1),           // b_max < b_min
            (0.1, 1.0, 0.0),           // zero step
            (0.1, 1.0, -0.1),          // negative step
            (f64::NAN, 1.0, 0.1),      // non-finite
            (0.1, f64::INFINITY, 0.1), // non-finite
            (0.1, 1.0, f64::NAN),      // non-finite
            (0.1, 1.0, 1.0),           // only 2 points
            (1e-3, 1e9, 1e-9),         // absurdly large grid
        ];
        for (lo, hi, step) in bad {
            assert!(
                matches!(build_grid(lo, hi, step), Err(FitError::Config(_))),
                "accepted ({lo}, {hi}, {step})"
            );
        }
    }

    // ---- find_candidates ----

    #[test]
    fn single_interior_minimum() {
        let g = pts(5);
        let c = find_candidates(&g, &[4.0, 2.0, 1.0, 3.0, 5.0]).unwrap();
        assert_eq!(
            c,
            vec![Candidate {
                left: 2.0,
                middle: 3.0,
                right: 4.0
            }]
        );
    }

    #[test]
    fn increasing_loss_gives_lower_boundary_only() {
        let g = pts(4);
        let c = find_candidates(&g, &[1.0, 2.0, 3.0, 4.0]).unwrap();
        assert_eq!(
            c,
            vec![Candidate {
                left: 1.0,
                middle: 1.0,
                right: 2.0
            }]
        );
    }

    #[test]
    fn decreasing_loss_gives_upper_boundary_only() {
        let g = pts(4);
        let c = find_candidates(&g, &[4.0, 3.0, 2.0, 1.0]).unwrap();
        assert_eq!(
            c,
            vec![Candidate {
                left: 3.0,
                middle: 4.0,
                right: 4.0
            }]
        );
    }

    #[test]
    fn two_interior_minima() {
        let g = pts(5);
        let c = find_candidates(&g, &[3.0, 1.0, 3.0, 2.0, 3.0]).unwrap();
        assert_eq!(c.len(), 2);
        assert_eq!(c[0].middle, 2.0);
        assert_eq!(c[1].middle, 4.0);
    }

    #[test]
    fn boundary_and_interior_minima_together() {
        let g = pts(4);
        let c = find_candidates(&g, &[1.0, 2.0, 1.0, 2.0]).unwrap();
        assert_eq!(c.len(), 2);
        assert_eq!(c[0].middle, 1.0); // lower boundary
        assert_eq!(c[1].middle, 3.0); // interior
    }

    #[test]
    fn both_boundaries_can_be_minima() {
        let g = pts(4);
        let c = find_candidates(&g, &[1.0, 3.0, 2.0, 1.0]).unwrap();
        assert_eq!(c.len(), 2);
        assert_eq!(c[0].middle, 1.0);
        assert_eq!(c[1].middle, 4.0);
    }

    #[test]
    fn flat_loss_falls_back_to_one_bracket() {
        let g = pts(3);
        let c = find_candidates(&g, &[1.0, 1.0, 1.0]).unwrap();
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn plateau_minimum_is_not_lost() {
        let g = pts(5);
        let c = find_candidates(&g, &[5.0, 2.0, 2.0, 5.0, 6.0]).unwrap();
        assert!(!c.is_empty());
    }

    #[test]
    fn rejects_mismatched_or_short_input() {
        assert!(find_candidates(&[1.0, 2.0, 3.0], &[1.0, 2.0]).is_err());
        assert!(find_candidates(&[1.0, 2.0], &[1.0, 2.0]).is_err());
    }
}
