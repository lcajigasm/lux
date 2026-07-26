use std::process::{Command, ExitCode};

fn run(program: &str, args: &[&str]) -> bool {
    println!("$ {program} {}", args.join(" "));
    let status = Command::new(program)
        .args(args)
        .status()
        .unwrap_or_else(|e| panic!("failed to spawn `{program}`: {e}"));
    status.success()
}

fn verify() -> ExitCode {
    let steps: &[(&str, &[&str])] = &[
        ("cargo", &["fmt", "--all", "--check"]),
        (
            "cargo",
            &[
                "clippy",
                "--workspace",
                "--all-targets",
                "--",
                "-D",
                "warnings",
            ],
        ),
        ("cargo", &["test", "--workspace"]),
        ("cargo", &["deny", "check"]),
        ("bash", &["./scripts/check-limits.sh"]),
        ("bash", &["./scripts/check-core-isolation.sh"]),
        ("bash", &["./scripts/check-bench-regression.sh", "main"]),
    ];

    for (program, args) in steps {
        if !run(program, args) {
            eprintln!("xtask verify: failed at `{program} {}`", args.join(" "));
            return ExitCode::FAILURE;
        }
    }
    println!("xtask verify: all checks passed.");
    ExitCode::SUCCESS
}

fn main() -> ExitCode {
    // Only one subcommand exists today; ignore args and just run it.
    verify()
}
