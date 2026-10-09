//! `bench_fixtures` — deterministic CrossHook bench fixture generator.
//!
//! Generate a canonical tree:
//!   cargo run -p crosshook-core --example bench_fixtures -- --seed 42 --out <dir> [--empty]
//! Materialize a canonical tree into an isolated XDG root (token -> real path):
//!   cargo run -p crosshook-core --example bench_fixtures -- materialize --from <dir> --root <dir>
//!
//! Same seed => byte-identical tree regardless of `--out`; different seed => different tree.
//! All output paths pass the safety guard (guard.rs) before any directory is created.

mod generate;
mod guard;
mod layout;
mod materialize;
mod normalize;
mod rng;

#[cfg(test)]
mod tests;

use std::path::PathBuf;
use std::process::ExitCode;

fn usage() -> &'static str {
    "usage:
  bench_fixtures --seed <u64> --out <dir> [--empty]
  bench_fixtures materialize --from <canonical-dir> --root <isolated-root>"
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("bench_fixtures: {error}\n{}", usage());
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        return Err("missing arguments".to_string());
    }

    if args[0] == "materialize" {
        let mut from: Option<PathBuf> = None;
        let mut root: Option<PathBuf> = None;
        let mut iter = args[1..].iter();
        while let Some(flag) = iter.next() {
            match flag.as_str() {
                "--from" => from = Some(PathBuf::from(iter.next().ok_or("--from needs a value")?)),
                "--root" => root = Some(PathBuf::from(iter.next().ok_or("--root needs a value")?)),
                other => return Err(format!("unknown materialize flag {other:?}")),
            }
        }
        let from = from.ok_or("materialize requires --from <canonical-dir>")?;
        let root = root.ok_or("materialize requires --root <isolated-root>")?;
        return materialize::materialize(&from, &root);
    }

    let mut seed: Option<u64> = None;
    let mut out: Option<PathBuf> = None;
    let mut empty = false;
    let mut iter = args.iter();
    while let Some(flag) = iter.next() {
        match flag.as_str() {
            "--seed" => {
                let raw = iter.next().ok_or("--seed needs a value")?;
                seed = Some(raw.parse().map_err(|e| format!("--seed {raw:?}: {e}"))?);
            }
            "--out" => out = Some(PathBuf::from(iter.next().ok_or("--out needs a value")?)),
            "--empty" => empty = true,
            other => return Err(format!("unknown flag {other:?}")),
        }
    }
    let out = out.ok_or("--out <dir> is required")?;
    let seed = seed.unwrap_or(42);
    let cfg = if empty {
        generate::BenchConfig::empty(seed)
    } else {
        generate::BenchConfig::full(seed)
    };
    generate::generate(&out, &cfg)
}
