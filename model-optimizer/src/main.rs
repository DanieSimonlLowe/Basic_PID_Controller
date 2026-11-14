//! Estimate motor parameters a, b for H(s) = a / (b s^2 + s) from a structlog file.
//!
//! Pipeline: readings → input steps (from `speed`) → coarse scan over b →
//! local-minimum brackets → basin::BrentDerivative per bracket → best fit.

mod collection_evaluator;
mod error;
mod fit;
mod grid;
mod input;
mod model;
mod optimizer;
mod plot;
mod readings;
#[cfg(test)]
mod test_support;

use std::path::PathBuf;

use clap::Parser;

use crate::fit::FitResult;
use crate::input::{Dataset, RunCollection};

#[derive(Parser, Debug)]
#[command(about = "Fit motor model H(s) = a / (b s^2 + s) from a structlog file")]
struct Args {
    /// structlog file containing Reading records
    #[arg(long)]
    input: PathBuf,
    /// structlog file containing Reading records
    #[arg(long)]
    confirm: PathBuf,
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
    #[arg(long, default_value_t = 0.04)]
    speed_tol: f64,
    /// Maximum BrentDerivative iterations per bracket
    #[arg(long, default_value_t = 200)]
    max_iter: u64,
    /// Where to write the result plot (SVG)
    #[arg(long, default_value = "fit.svg")]
    plot: PathBuf,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    let grid = grid::build_grid(args.b_min, args.b_max, args.b_step)?;

    let loaded = readings::load(&args.input, true)?;
    println!("Loaded {} readings", loaded.samples.len());

    let run_collection = RunCollection::new(&loaded.samples, &loaded.speeds, args.speed_tol)?;
    print_search_settings(&args);

    let evaluator = collection_evaluator::CollectionEvaluator::new(&run_collection)?;
    let result = fit::fit(&evaluator, &grid, args.max_iter)?;
    print_result(&result);

    let best = &result.best;
    let confirm_loaded = readings::load(&args.confirm, true)?;
    let confirm_dataset = Dataset::new(
        &confirm_loaded.samples,
        &confirm_loaded.speeds,
        args.speed_tol,
        0.0,
    )?;
    let confirm_evaluator = model::Evaluator::new(confirm_dataset);
    let predicted = confirm_evaluator.predict(best.b, best.eval.a);
    plot::plot_fit(
        confirm_evaluator.dataset(),
        &predicted,
        best.eval.a,
        best.b,
        &args.plot,
    )?;
    println!("\nPlot written to {}", args.plot.display());
    Ok(())
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
    println!("    a    = {:.9e}", best.eval.a);
    println!("    b    = {:.9e}", best.b);
    println!("    loss = {:.9e}", best.eval.loss);
    println!("    dL*/db at optimum = {:.3e}", best.eval.dloss);

    println!("\nBest bracket:");
    println!("    left   = {}", best.candidate.left);
    println!("    middle = {}", best.candidate.middle);
    println!("    right  = {}", best.candidate.right);
}
