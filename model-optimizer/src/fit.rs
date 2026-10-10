//! The full pipeline: coarse scan → brackets → basin refinement → best fit.

use crate::error::FitError;
use crate::grid::{find_candidates, Candidate};
use crate::model::{Eval, Evaluator};
use crate::optimizer::refine;

/// One refined local minimum.
#[derive(Debug, Clone, Copy)]
pub struct Refined {
    pub candidate: Candidate,
    pub b: f64,
    pub eval: Eval,
}

pub struct FitResult {
    /// The refined minimum with the lowest loss.
    pub best: Refined,
    /// All refined minima, in grid order.
    pub all: Vec<Refined>,
}

/// L*(b) at every grid point.
pub fn coarse_scan(evaluator: &Evaluator, grid: &[f64]) -> Result<Vec<f64>, FitError> {
    grid.iter()
        .map(|&b| evaluator.evaluate(b).map(|e| e.loss))
        .collect()
}

pub fn fit(evaluator: &Evaluator, grid: &[f64], max_iter: u64) -> Result<FitResult, FitError> {
    let losses = coarse_scan(evaluator, grid)?;
    let candidates = find_candidates(grid, &losses)?;

    let candidate_count = candidates.len();
    println!("found {} candidates.", candidate_count);
    let mut count = 0;
    let mut all = Vec::with_capacity(candidates.len());
    for candidate in candidates {
        let b = refine(evaluator, &candidate, max_iter)?;
        let eval = evaluator.evaluate(b)?;
        all.push(Refined { candidate, b, eval });
        count += 1;
        println!("{}/{}", count, candidate_count);
    }

    let best = all
        .iter()
        .min_by(|x, y| x.eval.loss.total_cmp(&y.eval.loss))
        .copied()
        .ok_or_else(|| FitError::Optimiser("no candidate minima were refined".into()))?;
    Ok(FitResult { best, all })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grid::build_grid;
    use crate::test_support::synthetic_dataset;

    #[test]
    fn recovers_true_parameters_from_noiseless_data() {
        let ev = Evaluator::new(synthetic_dataset(2.5, 0.3));
        let grid = build_grid(0.01, 2.0, 0.05).unwrap();
        let r = fit(&ev, &grid, 200).unwrap();
        assert!((r.best.b - 0.3).abs() < 1e-6, "b = {}", r.best.b);
        assert!((r.best.eval.a - 2.5).abs() < 1e-6, "a = {}", r.best.eval.a);
        assert!(r.best.eval.loss < 1e-6);
    }

    #[test]
    fn does_not_need_a_good_starting_guess() {
        // Different true b values, same wide grid.
        for &b_true in &[0.02, 0.15, 0.8, 3.0] {
            let ev = Evaluator::new(synthetic_dataset(1.7, b_true));
            let grid = build_grid(0.005, 6.0, 0.05).unwrap();
            let r = fit(&ev, &grid, 200).unwrap();
            assert!(
                (r.best.b - b_true).abs() < 1e-4 * b_true.max(1.0),
                "true {b_true}, got {}",
                r.best.b
            );
        }
    }

    #[test]
    fn tolerates_small_deterministic_noise() {
        let mut ds = synthetic_dataset(2.5, 0.3);
        for (i, y) in ds.outputs.iter_mut().enumerate() {
            *y += 0.05 * (i as f64).sin();
        }
        let ev = Evaluator::new(ds);
        let grid = build_grid(0.01, 2.0, 0.05).unwrap();
        let r = fit(&ev, &grid, 200).unwrap();
        assert!((r.best.b - 0.3).abs() < 0.01, "b = {}", r.best.b);
        assert!((r.best.eval.a - 2.5).abs() < 0.05, "a = {}", r.best.eval.a);
    }

    #[test]
    fn minimum_below_search_range_returns_lower_boundary() {
        let ev = Evaluator::new(synthetic_dataset(2.5, 0.3));
        let grid = build_grid(0.5, 2.0, 0.1).unwrap();
        let r = fit(&ev, &grid, 200).unwrap();
        assert!(r.best.b >= 0.5 && r.best.b < 0.5 + 1e-3, "b = {}", r.best.b);
    }

    #[test]
    fn best_is_the_lowest_loss_among_all_refined() {
        let ev = Evaluator::new(synthetic_dataset(2.5, 0.3));
        let grid = build_grid(0.01, 2.0, 0.05).unwrap();
        let r = fit(&ev, &grid, 200).unwrap();
        assert!(!r.all.is_empty());
        assert!(r.all.iter().all(|x| r.best.eval.loss <= x.eval.loss));
    }

    #[test]
    fn coarse_scan_has_one_loss_per_grid_point() {
        let ev = Evaluator::new(synthetic_dataset(2.5, 0.3));
        let grid = build_grid(0.01, 2.0, 0.05).unwrap();
        let losses = coarse_scan(&ev, &grid).unwrap();
        assert_eq!(losses.len(), grid.len());
        assert!(losses.iter().all(|l| l.is_finite() && *l >= 0.0));
    }
}
