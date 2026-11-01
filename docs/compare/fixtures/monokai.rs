// Monokai sample: keywords, strings, numbers, and a comment.
fn greet(name: &str) -> String {
    let times = 3;
    // ñ, emoji, and CJK stay in the buffer as UTF-8.
    format!("Hola {name} × {times} — 日本語")
}
