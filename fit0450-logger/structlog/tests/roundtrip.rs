use serde::{Deserialize, Serialize};
use std::fs::OpenOptions;
use std::io::Write;
use structlog::{Error, LogReader, LogWriter};

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
struct Item {
    id: u32,
    name: String,
    tags: Vec<String>,
    score: Option<f64>,
}

fn item(i: u32) -> Item {
    Item { id: i, name: format!("item-{i}"), tags: vec!["a".into(); (i % 4) as usize], score: (i % 2 == 0).then_some(i as f64) }
}

fn tmp(name: &str) -> std::path::PathBuf {
    let p = std::env::temp_dir().join(format!("structlog-test-{}-{name}", std::process::id()));
    let _ = std::fs::remove_file(&p);
    p
}

#[test]
fn roundtrip_and_reopen_append() {
    let p = tmp("rt");
    let mut w = LogWriter::<Item>::open(&p).unwrap();
    for i in 0..100 { w.append(&item(i)).unwrap(); }
    w.flush().unwrap();
    drop(w);

    let mut w = LogWriter::<Item>::open(&p).unwrap();
    for i in 100..150 { w.append(&item(i)).unwrap(); }
    drop(w); // flush-on-drop

    let got = structlog::read_all::<Item, _>(&p).unwrap();
    assert_eq!(got, (0..150).map(item).collect::<Vec<_>>());
}

#[test]
fn iterator_is_lazy_and_composable() {
    let p = tmp("lazy");
    let mut w = LogWriter::<Item>::open(&p).unwrap();
    for i in 0..1000 { w.append(&item(i)).unwrap(); }
    w.flush().unwrap();
    let firsts: Vec<_> = LogReader::<Item>::open(&p).unwrap().take(3).map(|r| r.unwrap().id).collect();
    assert_eq!(firsts, vec![0, 1, 2]);
}

#[test]
fn torn_tail_is_ignored_and_repaired() {
    let p = tmp("torn");
    let mut w = LogWriter::<Item>::open(&p).unwrap();
    for i in 0..5 { w.append(&item(i)).unwrap(); }
    w.flush().unwrap();
    drop(w);

    // Simulate a crash mid-record: header claims 100 bytes, only 3 present.
    let mut f = OpenOptions::new().append(true).open(&p).unwrap();
    f.write_all(&100u32.to_le_bytes()).unwrap();
    f.write_all(&0u32.to_le_bytes()).unwrap();
    f.write_all(&[1, 2, 3]).unwrap();
    drop(f);

    assert_eq!(structlog::read_all::<Item, _>(&p).unwrap().len(), 5);

    let mut w = LogWriter::<Item>::open(&p).unwrap();
    w.append(&item(5)).unwrap();
    drop(w);
    assert_eq!(structlog::read_all::<Item, _>(&p).unwrap(), (0..6).map(item).collect::<Vec<_>>());
}

#[test]
fn mid_file_corruption_is_reported() {
    let p = tmp("corrupt");
    let mut w = LogWriter::<Item>::open(&p).unwrap();
    for i in 0..5 { w.append(&item(i)).unwrap(); }
    drop(w);

    let mut bytes = std::fs::read(&p).unwrap();
    bytes[8 + 8 + 1] ^= 0xFF; // flip a payload byte in the first record
    std::fs::write(&p, bytes).unwrap();

    let mut r = LogReader::<Item>::open(&p).unwrap();
    assert!(matches!(r.next(), Some(Err(Error::Corrupt { .. }))));
    assert!(r.next().is_none());
    assert!(matches!(LogWriter::<Item>::open(&p), Err(Error::Corrupt { .. })));
}

#[test]
fn rejects_foreign_file() {
    let p = tmp("foreign");
    std::fs::write(&p, b"hello world, not a log").unwrap();
    assert!(matches!(LogReader::<Item>::open(&p), Err(Error::BadMagic)));
    assert!(matches!(LogWriter::<Item>::open(&p), Err(Error::BadMagic)));
}
