//! Evaluating the reduced problem over a whole [`RunCollection`].
//!
//! Every run (dataset) gets its own [`Evaluator`], so all of the model maths
//! stays in `model.rs`. The runs share the time constant `b` but each run has
//! its own optimal gain a_k*(b). Because dL_k/da_k = 0 at each run's optimum,
//! the combined reduced problem is simply
//!
//!   L*(b)    = sum_k L_k*(b)
//!   dL*/db   = sum_k dL_k*/db
//!
//! The per-run evaluations are independent, so with the `parallel` cargo
//! feature they are run on rayon's thread pool. Results are always summed in
//! run order, so the output does not depend on the thread count.

use crate::error::FitError;
use crate::input::RunCollection;
use crate::model::{Eval, Evaluator};

#[cfg(feature = "parallel")]
use rayon::prelude::*;

/// Anything the fitting pipeline can minimise over `b`.
pub trait Objective: Sync {
    fn evaluate(&self, b: f64) -> Result<Eval, FitError>;
}

impl Objective for Evaluator {
    fn evaluate(&self, b: f64) -> Result<Eval, FitError> {
        Evaluator::evaluate(self, b)
    }
}

pub struct CollectionEvaluator {
    evaluators: Vec<Evaluator>,
}

impl CollectionEvaluator {
    pub fn new(runs: &RunCollection) -> Result<Self, FitError> {
        if runs.datasets.is_empty() {
            return Err(FitError::Data("run collection has no datasets".into()));
        }
        // Requires `#[derive(Clone)]` on `Dataset` (the Arc is shared with the
        // collection, so the data is copied once here).
        let evaluators = runs
            .datasets
            .iter()
            .map(|ds| Evaluator::new((**ds).clone()))
            .collect();
        Ok(Self { evaluators })
    }

    /// Per-run results at `b`, in run order (e.g. to report each run's gain).
    pub fn evaluate_each(&self, b: f64) -> Result<Vec<Eval>, FitError> {
        #[cfg(feature = "parallel")]
        {
            self.evaluators
                .par_iter()
                .map(|ev| ev.evaluate(b))
                .collect()
        }
        #[cfg(not(feature = "parallel"))]
        {
            self.evaluators.iter().map(|ev| ev.evaluate(b)).collect()
        }
    }

    /// Combined result at `b`: losses and derivatives are summed over runs;
    /// `a` is the mean of the per-run gains (see [`Self::evaluate_each`] for
    /// the individual values).
    pub fn evaluate(&self, b: f64) -> Result<Eval, FitError> {
        let each = self.evaluate_each(b)?;
        let n = each.len() as f64;

        let (mut a, mut loss, mut dloss) = (0.0, 0.0, 0.0);
        for e in &each {
            a += e.a;
            loss += e.loss;
            dloss += e.dloss;
        }

        let out = Eval {
            a: a / n,
            loss,
            dloss,
        };
        if !(out.a.is_finite() && out.loss.is_finite() && out.dloss.is_finite()) {
            return Err(FitError::Numeric(format!("non-finite result at b = {b}")));
        }
        Ok(out)
    }
}

impl Objective for CollectionEvaluator {
    fn evaluate(&self, b: f64) -> Result<Eval, FitError> {
        CollectionEvaluator::evaluate(self, b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Build a collection from two copies of the synthetic data; the combined
    // loss must be the sum of the parts.
    #[test]
    fn combined_loss_is_sum_of_runs() {
        use crate::test_support::synthetic_dataset;
        let ev_a = Evaluator::new(synthetic_dataset(2.5, 0.3));
        let ev_b = Evaluator::new(synthetic_dataset(1.0, 0.3));
        let coll = CollectionEvaluator {
            evaluators: vec![
                Evaluator::new(synthetic_dataset(2.5, 0.3)),
                Evaluator::new(synthetic_dataset(1.0, 0.3)),
            ],
        };
        let b = 0.5;
        let (x, y) = (ev_a.evaluate(b).unwrap(), ev_b.evaluate(b).unwrap());
        let c = coll.evaluate(b).unwrap();
        assert_eq!(c.loss, x.loss + y.loss);
        assert_eq!(c.dloss, x.dloss + y.dloss);
    }
}
