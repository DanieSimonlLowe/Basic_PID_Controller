//! Estimate motor parameters a, b for H(s) = a / (b s^2 + s) from a structlog file.
//!
//! Pipeline: readings → input steps (from `speed`) → coarse scan over b →
//! local-minimum brackets → basin::BrentDerivative per bracket → best fit.

mod error;
mod fit;
mod grid;
mod input;
mod model;
mod optimizer;
mod readings;
#[cfg(test)]
mod test_support;

use std::path::PathBuf;

use clap::Parser;

use crate::fit::FitResult;
use crate::input::Dataset;

#[derive(Parser, Debug)]
#[command(about = "Fit motor model H(s) = a / (b s^2 + s) from a structlog file")]
struct Args {
    /// structlog file containing Reading records
    #[arg(long)]
    input: PathBuf,
    /// Lower bound of the b search (seconds, > 0)
    #[arg(long)]
    b_min: f64,
    /// Upper bound of the b search (seconds)
    #[arg(long)]
    b_max: f64,
    /// Coarse grid step for b (seconds)
    #[arg(long)]
    b_step: f64,
    /// Tolerance for detecting a change in `speed` (0 = exact comparison)
    #[arg(long, default_value_t = 0.0)]
    speed_tol: f64,
    /// Subtract the first count from all counts (the model assumes o(0) = 0)
    #[arg(long)]
    subtract_initial_count: bool,
    /// Maximum BrentDerivative iterations per bracket
    #[arg(long, default_value_t = 200)]
    max_iter: u64,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    let grid = grid::build_grid(args.b_min, args.b_max, args.b_step)?;

    let loaded = readings::load(&args.input, args.subtract_initial_count)?;
    println!("Loaded {} readings", loaded.samples.len());

    let dataset = Dataset::new(&loaded.samples, &loaded.speeds, args.speed_tol)?;
    print_input_summary(&dataset);
    print_search_settings(&args);

    let evaluator = model::Evaluator::new(dataset);
    let result = fit::fit(&evaluator, &grid, args.max_iter)?;
    print_result(&result);
    Ok(())
}

fn print_input_summary(data: &Dataset) {
    const SHOWN: usize = 5;
    let steps = &data.steps;
    println!("\nReconstructed input (from Reading.speed):");
    println!("    steps: {}", steps.len());
    println!("    first input value: {}", steps[0].value);
    println!("    last input value:  {}", steps[steps.len() - 1].value);
    for s in steps.iter().take(SHOWN) {
        println!(
            "    t = {:.6} s  w = {:+.6}  d = {:+.6}",
            s.time, s.value, s.delta
        );
    }
    if steps.len() > SHOWN {
        println!("    ... ({} more)", steps.len() - SHOWN);
    }
}

fn print_search_settings(args: &Args) {
    println!("\nb search:");
    println!("    minimum: {}", args.b_min);
    println!("    maximum: {}", args.b_max);
    println!("    step: {}", args.b_step);
}

fn print_result(result: &FitResult) {
    println!("\nCandidate minima found: {}", result.all.len());
    for r in &result.all {
        println!(
            "    bracket [{:.6}, {:.6}] -> b = {:.9}, loss = {:.6e}",
            r.candidate.left, r.candidate.right, r.b, r.eval.loss
        );
    }

    let best = &result.best;
    println!("\nBest fit:");
    println!("    a    = {:.9}", best.eval.a);
    println!("    b    = {:.9}", best.b);
    println!("    loss = {:.9e}", best.eval.loss);
    println!("    dL*/db at optimum = {:.3e}", best.eval.dloss);

    println!("\nBest bracket:");
    println!("    left   = {}", best.candidate.left);
    println!("    middle = {}", best.candidate.middle);
    println!("    right  = {}", best.candidate.right);
}
