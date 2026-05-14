use anyhow::Result;
use std::path::Path;
use surfacide_ast::FileRegistry;
use surfacide_syntax::Parser;

use crate::cmd_parse::collect_surf_files;

pub fn run_docs(path: &Path, out_dir: &Path) -> Result<()> {
    let mut files = FileRegistry::new();
    let mut parser = Parser::new();
    let mut parsed_files = Vec::new();

    let surf_files = collect_surf_files(path)?;
    if surf_files.is_empty() {
        eprintln!("no .surf files under {}", path.display());
        std::process::exit(2);
    }

    let mut parse_errs = 0usize;
    for fp in &surf_files {
        let src = std::fs::read_to_string(fp)?;
        let fid = files.add(fp.clone(), src.clone());
        let r = parser.parse(fid, &src);
        for d in &r.diagnostics {
            if d.is_error() {
                parse_errs += 1;
            }
        }
        if !r.diagnostics.is_empty() {
            surfacide_diag::render::print_all(&r.diagnostics, &files);
        }
        if let Some(m) = r.module {
            parsed_files.push((fid, m));
        }
    }

    if parse_errs > 0 {
        eprintln!("surfacide: {} parse error(s); refusing to emit docs", parse_errs);
        std::process::exit(1);
    }

    let resolved = surfacide_resolve::build_project(parsed_files, files.clone());
    let written = surfacide_docs::emit(&resolved.project, out_dir)?;

    eprintln!(
        "surfacide: wrote {} doc file(s) to {}",
        written.len(),
        out_dir.display()
    );
    for w in &written {
        eprintln!("  {}", w.display());
    }
    Ok(())
}
