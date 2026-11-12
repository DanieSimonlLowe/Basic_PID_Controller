//! Plot of the fitted model against the measured data.

use std::path::Path;

use plotters::prelude::*;

use crate::input::Dataset;

fn range(vals: impl Iterator<Item = f64>) -> (f64, f64) {
    let (lo, hi) = vals.fold((f64::INFINITY, f64::NEG_INFINITY), |(l, h), v| {
        (l.min(v), h.max(v))
    });
    let pad = ((hi - lo) * 0.05).max(1e-9);
    (lo - pad, hi + pad)
}

pub fn plot_fit(
    data: &Dataset,
    predicted: &[f64],
    a: f64,
    b: f64,
    path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let root = SVGBackend::new(path, (1000, 700)).into_drawing_area();
    root.fill(&WHITE)?;
    let (top, bottom) = root.split_vertically(450);

    let (t0, t1) = (data.times[0], *data.times.last().unwrap());

    // --- Output: measured vs model ---
    let (ymin, ymax) = range(data.outputs.iter().chain(predicted).copied());
    let mut chart = ChartBuilder::on(&top)
        .caption(format!("Fit: a = {a:.6}, b = {b:.6} s"), ("sans-serif", 22))
        .margin(10)
        .x_label_area_size(30)
        .y_label_area_size(60)
        .build_cartesian_2d(t0..t1, ymin..ymax)?;
    chart
        .configure_mesh()
        .x_desc("time (s)")
        .y_desc("output")
        .draw()?;

    chart
        .draw_series(LineSeries::new(
            data.times.iter().copied().zip(data.outputs.iter().copied()),
            &BLUE,
        ))?
        .label("measured")
        .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], BLUE));
    chart
        .draw_series(LineSeries::new(
            data.times.iter().copied().zip(predicted.iter().copied()),
            &RED,
        ))?
        .label("model")
        .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], RED));
    chart
        .configure_series_labels()
        .background_style(WHITE.mix(0.8))
        .border_style(BLACK)
        .draw()?;

    // --- Input: reconstructed staircase ---
    let (imin, imax) = range(data.steps.iter().map(|s| s.value));
    let mut stairs: Vec<(f64, f64)> = Vec::new();
    let mut prev = 0.0;
    for s in &data.steps {
        stairs.push((s.time, prev));
        stairs.push((s.time, s.value));
        prev = s.value;
    }
    stairs.push((t1, prev));

    let mut chart = ChartBuilder::on(&bottom)
        .margin(10)
        .x_label_area_size(30)
        .y_label_area_size(60)
        .build_cartesian_2d(t0..t1, imin..imax)?;
    chart
        .configure_mesh()
        .x_desc("time (s)")
        .y_desc("input")
        .draw()?;
    chart.draw_series(LineSeries::new(stairs, &GREEN))?;

    root.present()?;
    Ok(())
}
