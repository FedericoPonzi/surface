use anyhow::{Context, Result};
use std::path::Path;
use surfacide_ast::FileRegistry;
use surfacide_syntax::Parser;

pub fn run(path: &Path) -> Result<()> {
    let mut files = FileRegistry::new();
    let mut parser = Parser::new();

    let surf_files = collect_surf_files(path)?;
    if surf_files.is_empty() {
        eprintln!("no .surf files found under {}", path.display());
        std::process::exit(2);
    }

    let mut total_errors = 0usize;
    for file_path in &surf_files {
        let source = std::fs::read_to_string(file_path)
            .with_context(|| format!("reading {}", file_path.display()))?;
        let file_id = files.add(file_path.clone(), source.clone());
        let result = parser.parse(file_id, &source);
        if !result.diagnostics.is_empty() {
            surfacide_diag::render::print_all(&result.diagnostics, &files);
            let (errs, _) = surfacide_diag::render::summarise(&result.diagnostics);
            total_errors += errs;
        } else if let Some(module) = result.module.as_ref() {
            println!(
                "{}: parsed OK (module `{}`, {} decl(s))",
                file_path.display(),
                module.header.name.segments.iter()
                    .map(|s| s.as_str())
                    .collect::<Vec<_>>()
                    .join("."),
                module.decls.len(),
            );
        } else if result.cst_root_kind.is_some() {
            println!(
                "{}: parsed OK (CST root = `{}`; no top-level module)",
                file_path.display(),
                result.cst_root_kind.as_deref().unwrap_or("?"),
            );
        }
    }

    if total_errors > 0 {
        eprintln!("{} parse error(s)", total_errors);
        std::process::exit(1);
    }
    Ok(())
}

pub(crate) fn collect_surf_files(path: &Path) -> Result<Vec<std::path::PathBuf>> {
    let meta = std::fs::metadata(path)
        .with_context(|| format!("stat {}", path.display()))?;
    if meta.is_file() {
        return Ok(vec![path.to_path_buf()]);
    }
    let mut out = Vec::new();
    walk(path, &mut out)?;
    out.sort();
    Ok(out)
}

fn walk(dir: &Path, out: &mut Vec<std::path::PathBuf>) -> Result<()> {
    for entry in std::fs::read_dir(dir).with_context(|| format!("reading dir {}", dir.display()))? {
        let entry = entry?;
        let p = entry.path();
        // Resolve symlinks (the examples/ dir uses symlinks into the repo).
        let meta = std::fs::metadata(&p)
            .with_context(|| format!("stat {}", p.display()))?;
        if meta.is_dir() {
            walk(&p, out)?;
        } else if meta.is_file() && p.extension().and_then(|s| s.to_str()) == Some("surf") {
            out.push(p);
        }
    }
    Ok(())
}
