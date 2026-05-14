//! Build script: compile the vendored tree-sitter-surface C parser
//! and link it into this crate.

fn main() {
    let grammar_dir =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tree-sitter-surface");
    let src = grammar_dir.join("src");

    println!("cargo:rerun-if-changed={}", src.join("parser.c").display());
    println!(
        "cargo:rerun-if-changed={}",
        src.join("tree_sitter/parser.h").display()
    );

    let mut build = cc::Build::new();
    build.include(&src);
    build.file(src.join("parser.c"));

    // Suppress the upstream-generated parser's noisy warnings.
    build.flag_if_supported("-Wno-unused-but-set-variable");
    build.flag_if_supported("-Wno-trigraphs");
    build.flag_if_supported("-w");

    build.compile("tree-sitter-surface");
}
