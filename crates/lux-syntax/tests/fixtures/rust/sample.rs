// A tiny fixture exercising common Rust constructs for highlight coverage.
use std::collections::HashMap;

/// Doc comment on a struct.
struct Point {
    x: i32,
    y: i32,
}

impl Point {
    fn distance(&self, other: &Point) -> f64 {
        let dx = (self.x - other.x) as f64;
        let dy = (self.y - other.y) as f64;
        (dx * dx + dy * dy).sqrt()
    }
}

fn main() {
    let mut counts: HashMap<&str, i32> = HashMap::new();
    counts.insert("origin", 0);
    let message = "hello, world";
    println!("{message}");
    if counts.is_empty() {
        panic!("unreachable");
    }
}
