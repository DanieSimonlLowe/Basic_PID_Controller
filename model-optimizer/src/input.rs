//! Reconstructing the piecewise-constant input from the logged `speed` values.

use crate::error::FitError;
use crate::readings::Sample;

use std::sync::Arc;

const MIN_STATIONARY_ZERO_SAMPLES: usize = 100;
const MIN_NONZERO_INPUT_SAMPLES: usize = 1000;
const MAX_DIFF_STATIONARY: f64 = 0.00001;

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
#[derive(Clone)]
pub struct Dataset {
    /// Sample times (s), non-decreasing.
    pub times: Box<[f64]>,
    /// Measured outputs y(t).
    pub outputs: Box<[f64]>,
    /// Reconstructed input steps, in time order.
    pub steps: Box<[Step]>,
}

impl Dataset {
    pub fn new(
        samples: &[Sample],
        speeds: &[f64],
        speed_tol: f64,
        output_offset: f64,
    ) -> Result<Self, FitError> {
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
            outputs: samples.iter().map(|s| s.output - output_offset).collect(),
            steps: steps.into_boxed_slice(),
        })
    }
}

fn find_break_points(samples: &[Sample], speeds: &[f64], speed_tol: f64) -> Vec<usize> {
    let mut candidates: Vec<usize> = Vec::new();

    let mut stationary_zero_count: usize = 0;
    let mut nonzero_input_count: usize = 0;

    for (index, (sample, &speed)) in samples.iter().zip(speeds).enumerate() {
        // Treat inputs within speed_tol of zero as zero.
        if speed.abs() <= speed_tol {
            // Count consecutive zero-input measurements with an unchanged output.

            if index > 0 && (sample.output - samples[index - 1].output).abs() < MAX_DIFF_STATIONARY
            {
                stationary_zero_count += 1;
            }

            if stationary_zero_count >= MIN_STATIONARY_ZERO_SAMPLES
                && nonzero_input_count > MIN_NONZERO_INPUT_SAMPLES
            {
                candidates.push(index);
            }
        } else {
            stationary_zero_count = 0;
            nonzero_input_count += 1;
        }
    }

    println!(
        "There are {} candidate break points in the dataset",
        candidates.len()
    );

    // Ignore a final breakpoint if no subsequent non-zero input exists.
    // Otherwise, we could create a dataset containing only stationary measurements.
    let mut break_points = Vec::new();

    for (i, &break_index) in candidates.iter().enumerate() {
        let next_break = candidates.get(i + 1).copied().unwrap_or(samples.len());

        let has_input_after_break = speeds[break_index..next_break]
            .iter()
            .any(|speed| speed.abs() > speed_tol);

        if has_input_after_break {
            break_points.push(break_index);
        }
    }

    break_points
}

pub struct RunCollection {
    pub datasets: Box<[Arc<Dataset>]>,
}

impl RunCollection {
    pub fn new(samples: &[Sample], speeds: &[f64], speed_tol: f64) -> Result<Self, FitError> {
        if !speed_tol.is_finite() || speed_tol < 0.0 {
            return Err(FitError::Config(
                "speed tolerance must be finite and >= 0".into(),
            ));
        }

        if samples.len() != speeds.len() {
            return Err(FitError::Data("samples and speeds differ in length".into()));
        }

        if samples.is_empty() {
            return Err(FitError::Data("no samples provided".into()));
        }

        if samples
            .iter()
            .any(|s| !s.time.is_finite() || !s.output.is_finite())
            || speeds.iter().any(|s| !s.is_finite())
        {
            return Err(FitError::Data(
                "non-finite time, output, or input value".into(),
            ));
        }

        let break_points = find_break_points(samples, speeds, speed_tol);

        println!(
            "There are {} break points in the dataset",
            break_points.len()
        );

        let mut datasets: Vec<Arc<Dataset>> = Vec::new();
        let mut start: usize = 0;

        for break_index in break_points {
            // Include the breakpoint in the previous dataset.
            let end = break_index + 1;

            let output_offset = samples[start].output;

            let dataset = Dataset::new(
                &samples[start..end],
                &speeds[start..end],
                speed_tol,
                output_offset,
            )?;

            datasets.push(Arc::new(dataset));

            // Start the next dataset at the breakpoint.
            // This intentionally overlaps the breakpoint sample.
            start = break_index;
        }

        // Add the final dataset.
        let output_offset = samples[start].output;

        let dataset = Dataset::new(
            &samples[start..],
            &speeds[start..],
            speed_tol,
            output_offset,
        )?;

        datasets.push(Arc::new(dataset));

        Ok(Self {
            datasets: datasets.into_boxed_slice(),
        })
    }
}
