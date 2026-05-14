use anyhow::Result;
use std::path::PathBuf;
use surfacide_ast::FileRegistry;
use surfacide_syntax::Parser;

use crate::cmd_parse::collect_surf_files;

pub struct Args {
    pub path: PathBuf,
    pub slots: bool,
    pub obligations: bool,
    pub obligations_strict: bool,
    pub resolve: bool,
}

pub fn run(args: Args) -> Result<()> {
    // Default: run all passes.
    let run_all = !args.slots && !args.obligations && !args.resolve;
    let run_resolve = args.resolve || run_all;
    let run_slots = args.slots || run_all;
    let run_oblig = args.obligations || run_all;

    let mut files = FileRegistry::new();
    let mut parser = Parser::new();
    let mut parsed_files = Vec::new();

    let surf_files = collect_surf_files(&args.path)?;
    if surf_files.is_empty() {
        eprintln!("no .surf files under {}", args.path.display());
        std::process::exit(2);
    }

    let mut all_diags = Vec::new();
    for file_path in &surf_files {
        let source = std::fs::read_to_string(file_path)?;
        let file_id = files.add(file_path.clone(), source.clone());
        let result = parser.parse(file_id, &source);
        all_diags.extend(result.diagnostics);
        if let Some(module) = result.module {
            parsed_files.push((file_id, module));
        }
    }

    // Build the project and surface cross-file diagnostics
    // (duplicate surface block, duplicate action name, etc.) regardless
    // of which downstream pass we're running.
    let resolved = surfacide_resolve::build_project(parsed_files, files.clone());
    let project = resolved.project;
    if run_resolve {
        all_diags.extend(resolved.diagnostics);
    }

    if run_slots {
        let r = surfacide_check::run_slot_pass(&project);
        all_diags.extend(r.diagnostics);
        let r = surfacide_check::run_cross_slot(&project);
        all_diags.extend(r.diagnostics);
    }
    if run_oblig {
        let r = surfacide_obligations::run(&project, args.obligations_strict);
        all_diags.extend(r.diagnostics);
    }

    let (errors, warnings) = surfacide_diag::render::summarise(&all_diags);
    if !all_diags.is_empty() {
        surfacide_diag::render::print_all(&all_diags, &files);
    }

    if errors > 0 {
        eprintln!("surfacide: {} error(s), {} warning(s)", errors, warnings);
        std::process::exit(1);
    } else if warnings > 0 {
        eprintln!("surfacide: {} warning(s)", warnings);
    } else {
        eprintln!(
            "surfacide: {} file(s) checked, no diagnostics",
            surf_files.len()
        );
    }
    Ok(())
}
