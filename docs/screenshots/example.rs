//! Sample opened by `scripts/screenshots.sh`.
//! The screenshot job builds `lux` and captures this file in the real window.

fn greet(name: &str) -> String {
    format!("Hello, {name}")
}

fn main() {
    let message = greet("Lux");
    println!("{message}");
}
