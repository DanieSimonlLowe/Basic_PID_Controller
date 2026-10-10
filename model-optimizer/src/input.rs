//! Reconstructing the piecewise-constant input from the logged `speed` values.

use crate::error::FitError;
use crate::readings::Sample;

/// One step of the reconstructed input:  i(t) = sum(delta_i * u(t - time_i)).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Step {
    /// Timestamp (s) of the first sample showing the new value.
    pub time: f64,
    /// New input value w[i].
    pub value: f64,
    /// d[i] = w[i] - w[i-1], with d[0] = w[0].
    pub delta: f64,
}

/// Detect input steps. A new step starts when |speed - current level| > `tol`.
/// Comparing against the current level (not the previous sample) means small
/// jitter never accumulates into a false step. `tol = 0` is exact comparison.
pub fn detect_steps(samples: &[Sample], speeds: &[f64], tol: f64) -> Vec<Step> {
    let mut steps: Vec<Step> = Vec::new();
    for (sample, &speed) in samples.iter().zip(speeds) {
        let previous = steps.last().map(|s| s.value);
        let changed = match previous {
            None => true,
            Some(level) => (speed - level).abs() > tol,
        };
        if changed {
            steps.push(Step {
                time: sample.time,
                value: speed,
                delta: speed - previous.unwrap_or(0.0),
            });
        }
    }
    steps
}

/// Everything that does not depend on `b`, computed once.
pub struct Dataset {
    /// Sample times (s), non-decreasing.
    pub times: Vec<f64>,
    /// Measured outputs y(t).
    pub outputs: Vec<f64>,
    /// Reconstructed input steps, in time order.
    pub steps: Vec<Step>,
}

impl Dataset {
    pub fn new(samples: &[Sample], speeds: &[f64], speed_tol: f64) -> Result<Self, FitError> {
        if !speed_tol.is_finite() || speed_tol < 0.0 {
            return Err(FitError::Config(
                "speed tolerance must be finite and >= 0".into(),
            ));
        }
        if samples.len() != speeds.len() {
            return Err(FitError::Data("samples and speeds differ in length".into()));
        }
        if samples
            .iter()
            .any(|s| !s.time.is_finite() || !s.output.is_finite())
        {
            return Err(FitError::Data("non-finite time or output value".into()));
        }
        let steps = detect_steps(samples, speeds, speed_tol);
        if steps.len() < 2 {
            return Err(FitError::Data(
                "no input changes detected in `speed` (need at least 2 distinct levels)".into(),
            ));
        }
        Ok(Self {
            times: samples.iter().map(|s| s.time).collect(),
            outputs: samples.iter().map(|s| s.output).collect(),
            steps,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn samples(n: usize, dt: f64) -> Vec<Sample> {
        (0..n)
            .map(|k| Sample {
                time: k as f64 * dt,
                output: 0.0,
            })
            .collect()
    }

    #[test]
    fn spec_example_steps() {
        let speeds = [1.0, 1.0, 1.0, 3.0, 3.0, 3.0, -2.0, -2.0];
        let steps = detect_steps(&samples(8, 0.01), &speeds, 0.0);
        assert_eq!(steps.len(), 3);
        assert_eq!(
            [steps[0].value, steps[1].value, steps[2].value],
            [1.0, 3.0, -2.0]
        );
        assert_eq!(
            [steps[0].delta, steps[1].delta, steps[2].delta],
            [1.0, 2.0, -5.0]
        );
        assert_eq!(steps[0].time, 0.0);
        assert!((steps[1].time - 0.03).abs() < 1e-12);
        assert!((steps[2].time - 0.06).abs() < 1e-12);
    }

    #[test]
    fn deltas_sum_to_last_value() {
        let speeds = [0.5, 0.5, 2.0, 2.0, -1.0, 4.0, 4.0, 0.0];
        let steps = detect_steps(&samples(8, 0.1), &speeds, 0.0);
        let total: f64 = steps.iter().map(|s| s.delta).sum();
        assert!((total - 0.0).abs() < 1e-12);
        assert_eq!(steps.len(), 5);
    }

    #[test]
    fn zero_first_value_still_produces_a_step() {
        let speeds = [0.0, 0.0, 2.0];
        let steps = detect_steps(&samples(3, 0.1), &speeds, 0.0);
        assert_eq!(steps.len(), 2);
        assert_eq!(steps[0].delta, 0.0);
        assert_eq!(steps[1].delta, 2.0);
    }

    #[test]
    fn exact_comparison_sees_tiny_changes() {
        let speeds = [1.0, 1.0001, 1.0001];
        assert_eq!(detect_steps(&samples(3, 0.1), &speeds, 0.0).len(), 2);
    }

    #[test]
    fn tolerance_suppresses_jitter_but_not_real_steps() {
        let speeds = [1.0, 1.01, 0.99, 1.02, 3.0, 3.01];
        let steps = detect_steps(&samples(6, 0.1), &speeds, 0.05);
        assert_eq!(steps.len(), 2);
        assert_eq!(steps[1].value, 3.0);
        assert!((steps[1].delta - 2.0).abs() < 1e-12);
    }

    #[test]
    fn dataset_rejects_constant_input() {
        let res = Dataset::new(&samples(5, 0.1), &[2.0; 5], 0.0);
        assert!(matches!(res, Err(FitError::Data(_))));
    }

    #[test]
    fn dataset_rejects_length_mismatch_and_bad_tolerance() {
        assert!(matches!(
            Dataset::new(&samples(5, 0.1), &[1.0, 2.0], 0.0),
            Err(FitError::Data(_))
        ));
        for tol in [-1.0, f64::NAN, f64::INFINITY] {
            assert!(matches!(
                Dataset::new(&samples(3, 0.1), &[1.0, 2.0, 3.0], tol),
                Err(FitError::Config(_))
            ));
        }
    }

    #[test]
    fn dataset_rejects_non_finite_output() {
        let mut s = samples(3, 0.1);
        s[1].output = f64::NAN;
        assert!(matches!(
            Dataset::new(&s, &[1.0, 2.0, 3.0], 0.0),
            Err(FitError::Data(_))
        ));
    }
}
