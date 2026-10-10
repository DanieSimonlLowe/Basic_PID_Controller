//! Test-only helpers: a noiseless dataset generated from the model itself.

use crate::input::Dataset;
use crate::model::response;
use crate::readings::Sample;

/// (start time in s, input level). The input is piecewise constant.
const LEVELS: [(f64, f64); 5] = [(0.0, 1.0), (0.5, 3.0), (1.2, -2.0), (2.0, 0.5), (3.1, 2.0)];

/// 500 samples at 10 ms spacing; output y(t) = a * S(t, b) exactly.
pub fn synthetic_dataset(a: f64, b: f64) -> Dataset {
    let n = 500;
    let dt = 0.01;

    let mut samples = Vec::with_capacity(n);
    let mut speeds = Vec::with_capacity(n);
    for k in 0..n {
        let t = k as f64 * dt;
        let level = LEVELS
            .iter()
            .rev()
            .find(|(start, _)| t >= *start)
            .map_or(0.0, |(_, v)| *v);
        samples.push(Sample {
            time: t,
            output: 0.0,
        });
        speeds.push(level);
    }

    let mut ds = Dataset::new(&samples, &speeds, 0.0).expect("synthetic data is valid");
    let outputs: Vec<f64> = ds
        .times
        .iter()
        .map(|&t| a * response(&ds.steps, t, b).0)
        .collect();
    ds.outputs = outputs;
    ds
}
