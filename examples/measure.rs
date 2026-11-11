//! In-process timing only; compilation/startup are measured by the caller.
use std::{hint::black_box, time::Instant};
fn main() {
    let path = std::env::args()
        .nth(1)
        .expect("measure CORPUS.json [ITERATIONS]");
    let iterations: usize = std::env::args()
        .nth(2)
        .unwrap_or("100".into())
        .parse()
        .unwrap();
    assert!(iterations > 0);
    let bytes = std::fs::read(path).unwrap();
    let start = Instant::now();
    let corpus: Vec<serde_json::Value> = serde_json::from_slice(&bytes).unwrap();
    let parse_us = start.elapsed().as_micros();
    let render = || {
        let mut total = 0;
        for request in &corpus {
            let output = bendec2rs::generate_string(
                request["types"].as_array().unwrap(),
                &request["options"],
            )
            .unwrap();
            total += black_box(output).len();
        }
        total
    };
    let expected = render(); // warmup
    let start = Instant::now();
    for _ in 0..iterations {
        assert_eq!(render(), expected);
    }
    println!("{{\"parse_us\":{parse_us},\"generation_us\":{},\"iterations\":{iterations},\"bytes_per_iteration\":{expected}}}", start.elapsed().as_micros());
}
