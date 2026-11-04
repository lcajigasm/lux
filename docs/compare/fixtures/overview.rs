//! A long enough sample that the minimap shows the shape of the file:
//! comments, strings, and indented blocks, not one gray bar per line.

const GREETING: &str = "hello from the overview fixture";

pub struct Lane {
    pub name: &'static str,
    pub width: u32,
}

pub fn lanes() -> Vec<Lane> {
    vec![
        Lane {
            name: "left",
            width: 18,
        },
        Lane {
            name: "center",
            width: 42,
        },
        Lane {
            name: "right",
            width: 18,
        },
    ]
}

/// Fold a label into the lane that should carry it.
pub fn place(label: &str, lanes: &[Lane]) -> Option<&Lane> {
    // Blank labels stay out of every lane.
    if label.trim().is_empty() {
        return None;
    }
    let mut best = None;
    let mut best_fit = 0usize;
    for lane in lanes {
        let fit = if label.len() <= lane.width as usize {
            lane.width as usize - label.len()
        } else {
            0
        };
        if fit > best_fit {
            best = Some(lane);
            best_fit = fit;
        }
    }
    best
}

pub fn render(lanes: &[Lane]) -> String {
    let mut out = String::from("/* overview\n");
    for lane in lanes {
        out.push_str(&format!("  {}:{}\n", lane.name, lane.width));
    }
    out.push_str("*/\n");
    out
}

fn describe(lane: &Lane) -> String {
    match lane.name {
        "left" => format!("dock {GREETING}"),
        "right" => "notes and the map".to_string(),
        _ => format!("{} × {}", lane.name, lane.width),
    }
}

fn main() {
    let lanes = lanes();
    println!("{}", render(&lanes));
    for lane in &lanes {
        println!("{} -> {}", lane.name, describe(lane));
        if let Some(home) = place(lane.name, &lanes) {
            println!("  home {}", home.name);
        }
    }
}
