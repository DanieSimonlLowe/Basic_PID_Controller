//! Reading the structlog file and converting it to seconds / f64.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::FitError;

/// Record layout used by the logger (must match the writer exactly).
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Reading {
    pub now_us: u64,
    pub count: i32,
    pub speed: f32,
}

/// One measured output point.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sample {
    /// Seconds since the first reading.
    pub time: f64,
    /// Measured output (encoder count) as f64.
    pub output: f64,
}

/// Samples plus the matching logged input (`speed`) values, both in time order.
pub struct Loaded {
    pub samples: Vec<Sample>,
    pub speeds: Vec<f64>,
}

/// Read a structlog file and prepare it for fitting.
pub fn load<P: AsRef<Path>>(path: P, subtract_initial_count: bool) -> Result<Loaded, FitError> {
    let readings =
        structlog::read_all::<Reading, _>(path).map_err(|e| FitError::Log(e.to_string()))?;
    prepare(readings, subtract_initial_count)
}

/// Validate readings, sort by timestamp, convert to seconds relative to the
/// first reading. Separate from `load` so it can be tested without a file.
pub fn prepare(
    mut readings: Vec<Reading>,
    subtract_initial_count: bool,
) -> Result<Loaded, FitError> {
    if readings.is_empty() {
        return Err(FitError::Data("log contains no readings".into()));
    }
    if readings.len() < 3 {
        return Err(FitError::Data("need at least 3 readings".into()));
    }

    // Stable sort: readings with equal timestamps keep their log order.
    readings.sort_by_key(|r| r.now_us);

    let first_us = readings[0].now_us;
    let first_count = f64::from(readings[0].count);

    let mut samples = Vec::with_capacity(readings.len());
    let mut speeds = Vec::with_capacity(readings.len());
    for r in &readings {
        if !r.speed.is_finite() {
            return Err(FitError::Data(format!(
                "non-finite speed at now_us = {}",
                r.now_us
            )));
        }
        let time = (r.now_us - first_us) as f64 / 1e6;
        let mut output = f64::from(r.count);
        if subtract_initial_count {
            output -= first_count;
        }
        samples.push(Sample { time, output });
        speeds.push(f64::from(r.speed));
    }

    let span = samples.last().map_or(0.0, |s| s.time);
    if span <= 0.0 {
        return Err(FitError::Data("log spans zero time".into()));
    }
    Ok(Loaded { samples, speeds })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(now_us: u64, count: i32, speed: f32) -> Reading {
        Reading {
            now_us,
            count,
            speed,
        }
    }

    #[test]
    fn times_are_relative_seconds() {
        let loaded = prepare(
            vec![r(500_000, 0, 1.0), r(600_000, 5, 1.0), r(1_500_000, 9, 2.0)],
            false,
        )
        .unwrap();
        assert_eq!(loaded.samples[0].time, 0.0);
        assert!((loaded.samples[1].time - 0.1).abs() < 1e-15);
        assert!((loaded.samples[2].time - 1.0).abs() < 1e-15);
    }

    #[test]
    fn outputs_and_speeds_are_converted() {
        let loaded = prepare(vec![r(0, 10, 0.5), r(10, -20, 1.5), r(20, 30, -2.0)], false).unwrap();
        assert_eq!(loaded.samples[1].output, -20.0);
        assert_eq!(loaded.speeds, vec![0.5, 1.5, -2.0]);
    }

    #[test]
    fn subtract_initial_count_shifts_outputs() {
        let loaded = prepare(vec![r(0, 100, 1.0), r(10, 130, 1.0), r(20, 90, 1.0)], true).unwrap();
        let outs: Vec<f64> = loaded.samples.iter().map(|s| s.output).collect();
        assert_eq!(outs, vec![0.0, 30.0, -10.0]);
    }

    #[test]
    fn unsorted_readings_are_sorted_by_time() {
        let loaded = prepare(
            vec![r(2_000, 2, 3.0), r(0, 0, 1.0), r(1_000, 1, 2.0)],
            false,
        )
        .unwrap();
        assert_eq!(loaded.speeds, vec![1.0, 2.0, 3.0]);
        assert_eq!(loaded.samples[0].time, 0.0);
    }

    #[test]
    fn rejects_empty_and_short_logs() {
        assert!(matches!(prepare(vec![], false), Err(FitError::Data(_))));
        assert!(matches!(
            prepare(vec![r(0, 0, 1.0), r(1, 1, 1.0)], false),
            Err(FitError::Data(_))
        ));
    }

    #[test]
    fn rejects_non_finite_speed() {
        for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let res = prepare(vec![r(0, 0, 1.0), r(1, 1, bad), r(2, 2, 1.0)], false);
            assert!(
                matches!(res, Err(FitError::Data(_))),
                "speed {bad} accepted"
            );
        }
    }

    #[test]
    fn rejects_zero_time_span() {
        let res = prepare(vec![r(5, 0, 1.0), r(5, 1, 1.0), r(5, 2, 1.0)], false);
        assert!(matches!(res, Err(FitError::Data(_))));
    }
}
