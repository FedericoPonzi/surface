use anyhow::Result;
use camino::Utf8PathBuf;
use clap::{Parser, Subcommand};

mod cmd_check;
mod cmd_emit;
mod cmd_parse;

/// Surfacide — the Surface specification language frontend.
///
/// Subcommands:
///   parse        parse a `.surf` file and print AST or errors
///   check        run frontend passes (slots, obligations) against a project
///   emit docs    project markdown documentation from a project
#[derive(Parser, Debug)]
#[command(name = "surfacide", version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Parse a `.surf` file and report syntax errors.
    Parse {
        /// File or directory to parse.
        path: Utf8PathBuf,
    },
    /// Run frontend checks against a project (directory of `.surf` files).
    Check {
        /// Project root to check.
        path: Utf8PathBuf,
        /// Run only the slot pass (§6.4).
        #[arg(long)]
        slots: bool,
        /// Run only the obligation pass (§15).
        #[arg(long)]
        obligations: bool,
        /// Promote medium-severity obligations to errors.
        #[arg(long)]
        obligations_strict: bool,
        /// Run only name resolution.
        #[arg(long)]
        resolve: bool,
    },
    /// Emit artefacts (currently: docs).
    Emit {
        #[command(subcommand)]
        what: EmitTarget,
    },
}

#[derive(Subcommand, Debug)]
enum EmitTarget {
    /// Emit Markdown docs from a project.
    Docs {
        /// Project root.
        path: Utf8PathBuf,
        /// Output directory.
        #[arg(short, long)]
        output: Utf8PathBuf,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Parse { path } => cmd_parse::run(path.as_std_path()),
        Command::Check {
            path,
            slots,
            obligations,
            obligations_strict,
            resolve,
        } => cmd_check::run(cmd_check::Args {
            path: path.into_std_path_buf(),
            slots,
            obligations,
            obligations_strict,
            resolve,
        }),
        Command::Emit { what } => match what {
            EmitTarget::Docs { path, output } => {
                cmd_emit::run_docs(path.as_std_path(), output.as_std_path())
            }
        },
    }
}
