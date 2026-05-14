//! Examples-must-pass gate.
//!
//! Runs the real parser against every committed `.surf` example.
//! These are the canonical worked specs the language ships with —
//! a parse regression on any of them is a release blocker.

use std::path::Path;
use surfacide_ast::{FileId, FileRegistry};
use surfacide_syntax::Parser;

const EXAMPLE_ROOTS: &[&str] = &[
    "examples/url-shortener",
    "examples/twitter",
];

fn workspace_root() -> std::path::PathBuf {
    // The integration test runs from the crate dir; the workspace root
    // is two dirs up.
    let here = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    here.parent().unwrap().parent().unwrap().to_path_buf()
}

fn collect_surf_files(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    walk(dir, &mut out);
    out.sort();
    out
}

fn walk(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let p = entry.path();
        let Ok(meta) = std::fs::metadata(&p) else { continue };
        if meta.is_dir() {
            walk(&p, out);
        } else if meta.is_file() && p.extension().and_then(|s| s.to_str()) == Some("surf") {
            out.push(p);
        }
    }
}

#[test]
fn every_v10_example_parses() {
    let root = workspace_root();
    let mut parser = Parser::new();
    let mut total = 0usize;
    let mut failures: Vec<String> = Vec::new();

    for rel in EXAMPLE_ROOTS {
        let dir = root.join(rel);
        let files = collect_surf_files(&dir);
        assert!(
            !files.is_empty(),
            "no .surf files found under {}",
            dir.display()
        );
        for file in files {
            total += 1;
            let source = std::fs::read_to_string(&file).expect("read file");
            let mut reg = FileRegistry::new();
            let id = reg.add(file.clone(), source.clone());
            let result = parser.parse(id, &source);
            if !result.diagnostics.is_empty() {
                failures.push(format!(
                    "{}: {} diagnostic(s) — first: {}",
                    file.display(),
                    result.diagnostics.len(),
                    result.diagnostics[0].message
                ));
            }
        }
    }

    if !failures.is_empty() {
        panic!(
            "{}/{} example file(s) failed to parse:\n  {}",
            failures.len(),
            total,
            failures.join("\n  ")
        );
    }
    eprintln!("{} v0.10-era example file(s) parse cleanly", total);
}
