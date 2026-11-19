use std::env;
use std::f64::consts::PI;
use std::process;

fn usage() -> ! {
    eprintln!("Usage: solver <a> <b> <Mp> <Tr> [steps]");
    eprintln!("  Mp    max overshoot, 0 < Mp < 1");
    eprintln!("  Tr    rise time, > 0");
    eprintln!("  steps number of grid steps (default 1000000)");
    process::exit(1);
}

fn parse(args: &[String], i: usize) -> f64 {
    args.get(i)
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or_else(|| usage())
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 5 {
        usage();
    }
    let a = parse(&args, 1);
    let b = parse(&args, 2);
    let mp_spec = parse(&args, 3);
    let tr_spec = parse(&args, 4);
    let steps: u64 = args
        .get(5)
        .map(|s| s.parse().unwrap_or_else(|_| usage()))
        .unwrap_or(1_000_000);

    if a == 0.0 || b <= 0.0 || !(mp_spec > 0.0 && mp_spec < 1.0) || tr_spec <= 0.0 || steps == 0 {
        eprintln!("Invalid input: need a != 0, b > 0, 0 < Mp < 1, Tr > 0, steps > 0");
        process::exit(1);
    }

    // z range: |ln(Mp)| / sqrt(ln(Mp)^2 + pi^2) <= z < 1
    let ln_mp = mp_spec.ln().abs();
    let z_min = ln_mp / (ln_mp * ln_mp + PI * PI).sqrt();
    let z_max = 1.0 - 1e-12; // strictly below 1 to avoid division by zero

    for i in 0..=steps {
        let z = z_min + (z_max - z_min) * (i as f64) / (steps as f64);
        let s = (1.0 - z * z).sqrt();

        // Solve for w: w = (pi - atan(sqrt(1-z^2)/z)) / (Tr * sqrt(1-z^2))
        let w = (PI - (s / z).atan()) / (tr_spec * s);

        // Check w*z >= 1/b
        if w * z >= 1.0 / b {
            let c = (b * w * z - 1.0) / a;
            let d = (b * w * w) / a;

            // Actual Mp and Tr at this point
            let mp = (-PI * z / s).exp();
            let tr = (PI - (s / z).atan()) / (w * s);

            println!("C: {}", c);
            println!("D: {}", d);
            println!("W: {}", w);
            println!("Z: {}", z);
            println!("Mp: {}", mp);
            println!("Tr: {}", tr);
            return;
        }
    }

    eprintln!("No valid point found on the grid.");
    process::exit(2);
}
