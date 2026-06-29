//! `qdl-check`: parse, lower, and validate a QDL file.

use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let Some(path) = std::env::args().nth(1).map(PathBuf::from) else {
        eprintln!("usage: qdl-check <file.qdl>");
        return ExitCode::FAILURE;
    };
    match quent_qdl::load(&path) {
        Ok(schema) => {
            println!(
                "ok: model `{}` ({} entities, {} records)",
                schema.name(),
                schema.entities().count(),
                schema.records().count(),
            );
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("{}: {e}", path.display());
            ExitCode::FAILURE
        }
    }
}
