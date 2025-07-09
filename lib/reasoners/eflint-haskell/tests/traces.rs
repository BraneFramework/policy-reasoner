//  TRACES.rs
//    by Lut99
//
//  Created:
//    25 Apr 2025, 17:18:10
//  Last edited:
//    25 Apr 2025, 17:23:15
//  Auto updated?
//    Yes
//
//  Description:
//!   Test for running the full trace parser on various files.
//

use std::fs::{self, ReadDir};
use std::path::PathBuf;
use std::str::FromStr as _;

use eflint_haskell_reasoner::trace::Trace;
use share::formatters::BlockFormatter;

/***** Tests *****/
#[test]
fn test_all_trace_files() {
    // Visit all traces
    let traces_path: PathBuf = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("traces");
    let entries: ReadDir =
        fs::read_dir(&traces_path).unwrap_or_else(|err| panic!("Failed to read traces directory {path}: {err}", path = traces_path.display()));

    for (i, entry) in entries.enumerate() {
        let entry = entry.unwrap_or_else(|err| panic!("Failed to read entry {i} in traces directory {path}: {err}", path = traces_path.display()));

        // Load the file
        let trace: String =
            fs::read_to_string(entry.path()).unwrap_or_else(|err| panic!("Failed to read trace file {path}: {err}", path = entry.path().display()));

        // Attempt to parse it
        if let Err(err) = Trace::from_str(&trace) {
            panic!(
                "Failed to parse trace of trace file {path}: {err}\n\n{trace}\n",
                path = entry.path().display(),
                trace = BlockFormatter::new("trace", trace),
            );
        }
    }
}
