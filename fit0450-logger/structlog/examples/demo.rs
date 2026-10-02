use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct Reading {
    sensor: String,
    value: f64,
    ts: u64,
}

fn main() -> structlog::Result<()> {
    let path = std::env::temp_dir().join("readings.slog");

    // Append (can be called across many runs; the file just grows).
    let mut w = structlog::LogWriter::<Reading>::open(&path)?;
    for i in 0..3 {
        w.append(&Reading { sensor: "temp".into(), value: 20.0 + i as f64, ts: i })?;
    }
    w.sync()?;

    // Read back lazily as an iterator.
    for r in structlog::iter::<Reading, _>(&path)? {
        println!("{:?}", r?);
    }
    Ok(())
}
