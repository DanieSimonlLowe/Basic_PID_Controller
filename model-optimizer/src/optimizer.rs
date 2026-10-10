//! Refining one bracket with basin's `BrentDerivative` (basin 1.4.0 API).

use basin::{
    BoxConstraints, BrentDerivative, CostFunction, Executor, Gradient, ScalarGradientState,
};

use crate::error::FitError;
use crate::grid::Candidate;
use crate::model::Evaluator;

/// The reduced objective L*(b) restricted to one bracket [lo, hi].
/// cost = L*(b), gradient = dL*/db, bounds = the bracket.
struct BracketProblem<'a> {
    evaluator: &'a Evaluator,
    lo: f64,
    hi: f64,
}

impl CostFunction for BracketProblem<'_> {
    type Param = f64;
    type Output = f64;
    type Error = FitError;

    fn cost(&self, b: &f64) -> Result<f64, FitError> {
        Ok(self.evaluator.evaluate(*b)?.loss)
    }
}

impl Gradient for BracketProblem<'_> {
    type Gradient = f64;

    fn gradient(&self, b: &f64) -> Result<f64, FitError> {
        Ok(self.evaluator.evaluate(*b)?.dloss)
    }
}

impl BoxConstraints for BracketProblem<'_> {
    fn lower(&self) -> &f64 {
        &self.lo
    }
    fn upper(&self) -> &f64 {
        &self.hi
    }
}

/// Minimise L*(b) inside `candidate.left ..= candidate.right`, seeded at
/// `candidate.middle`. Returns the best `b` found (always inside the bracket).
pub fn refine(
    evaluator: &Evaluator,
    candidate: &Candidate,
    max_iter: u64,
) -> Result<f64, FitError> {
    let problem = BracketProblem {
        evaluator,
        lo: candidate.left,
        hi: candidate.right,
    };
    let state = ScalarGradientState::new(candidate.middle);
    // An Err from the problem (e.g. invalid b) is returned untouched by basin.
    let result = Executor::new(problem, BrentDerivative::new(), state)
        .max_iter(max_iter as _)
        .run()?;

    let b: f64 = result.best_param().to_owned();
    if !b.is_finite() || b < candidate.left || b > candidate.right {
        return Err(FitError::Optimiser(format!(
            "basin returned b = {b} outside bracket [{}, {}] (reason: {:?})",
            candidate.left, candidate.right, result.reason
        )));
    }
    Ok(b)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::synthetic_dataset;

    fn evaluator() -> Evaluator {
        Evaluator::new(synthetic_dataset(2.5, 0.3))
    }

    #[test]
    fn finds_interior_minimum() {
        let ev = evaluator();
        let c = Candidate {
            left: 0.2,
            middle: 0.25,
            right: 0.4,
        };
        let b = refine(&ev, &c, 200).unwrap();
        assert!((b - 0.3).abs() < 1e-6, "b = {b}");
    }

    #[test]
    fn seed_position_does_not_matter() {
        let ev = evaluator();
        for middle in [0.21, 0.3, 0.39] {
            let c = Candidate {
                left: 0.2,
                middle,
                right: 0.4,
            };
            let b = refine(&ev, &c, 200).unwrap();
            assert!((b - 0.3).abs() < 1e-6, "seed {middle} -> b = {b}");
        }
    }

    #[test]
    fn minimum_outside_bracket_converges_to_boundary() {
        let ev = evaluator();
        // True b = 0.3 lies below this bracket; L* increases across it.
        let c = Candidate {
            left: 0.5,
            middle: 0.6,
            right: 0.7,
        };
        let b = refine(&ev, &c, 200).unwrap();
        assert!(b >= 0.5 && b < 0.5 + 1e-3, "b = {b}");
    }

    #[test]
    fn problem_errors_are_propagated() {
        let ev = evaluator();
        // Bracket reaches b <= 0, which the evaluator rejects.
        let c = Candidate {
            left: -1.0,
            middle: 0.0,
            right: 1.0,
        };
        assert!(refine(&ev, &c, 50).is_err());
    }
}
