//  TESTS.rs
//    by Lut99
//
//  Created:
//    31 Oct 2023, 15:27:38
//  Last edited:
//    01 Nov 2023, 14:10:56
//  Auto updated?
//    Yes
//
//  Description:
//!   Implements tests for the [`Workflow`](super::spec::Workflow) (or
//!   rather, its compiler(s)).
//

use std::ffi::OsStr;
use std::path::PathBuf;

use brane_ast::{ast, compile_program, CompileResult, ParserOptions};
use brane_shr::utilities::{create_data_index_from, create_package_index_from, test_on_dsl_files_in};
use specifications::data::DataIndex;
use specifications::package::PackageIndex;

use super::spec::Workflow;


/***** CONSTANTS *****/
/// Defines the location of the tests
pub const TESTS_DIR: &str = "../../tests";





/***** LIBRARY *****/
/// Run all the BraneScript tests
#[test]
fn test_checker_workflow() {
    let tests_path: PathBuf = PathBuf::from(TESTS_DIR);

    // Run the compiler for every applicable DSL file
    test_on_dsl_files_in("BraneScript", &tests_path, |path: PathBuf, code: String| {
        // Start by the name to always know which file this is
        println!("{}", (0..80).map(|_| '-').collect::<String>());
        println!("File '{}' gave us:", path.display());

        // Skip some files, sadly
        if let Some(name) = path.file_name() {
            if name == OsStr::new("class.bs") {
                println!("Skipping test, since instance calling is not supported in checker workflows...");
                println!("{}\n\n", (0..80).map(|_| '-').collect::<String>());
                return;
            }
        }

        // Load the package index
        let pindex: PackageIndex = create_package_index_from(tests_path.join("packages"));
        let dindex: DataIndex = create_data_index_from(tests_path.join("data"));

        // Compile the raw source to WIR
        let wir: ast::Workflow = match compile_program(code.as_bytes(), &pindex, &dindex, &ParserOptions::bscript()) {
            CompileResult::Workflow(wir, warns) => {
                // Print warnings if any
                for w in warns {
                    w.prettyprint(path.to_string_lossy(), &code);
                }
                wir
            },
            CompileResult::Eof(err) => {
                // Print the error
                err.prettyprint(path.to_string_lossy(), &code);
                panic!("Failed to compile to WIR (see output above)");
            },
            CompileResult::Err(errs) => {
                // Print the errors
                for e in errs {
                    e.prettyprint(path.to_string_lossy(), &code);
                }
                panic!("Failed to compile to WIR (see output above)");
            },

            _ => {
                unreachable!();
            },
        };

        // Next, compile to the checker's workflow
        let wf: Workflow = match wir.try_into() {
            Ok(wf) => wf,
            Err(err) => {
                panic!("Failed to compile WIR to CheckerWorkflow: {err}");
            },
        };

        // Now print the file for prettyness
        println!("{}", wf.visualize());
        println!("{}\n\n", (0..80).map(|_| '-').collect::<String>());
    });
}

/// Run all the BraneScript tests _with_ optimization
#[test]
fn test_checker_workflow_optimized() {
    let tests_path: PathBuf = PathBuf::from(TESTS_DIR);

    // Run the compiler for every applicable DSL file
    test_on_dsl_files_in("BraneScript", &tests_path, |path: PathBuf, code: String| {
        // Start by the name to always know which file this is
        println!("{}", (0..80).map(|_| '-').collect::<String>());
        println!("(Optimized) File '{}' gave us:", path.display());

        // Skip some files, sadly
        if let Some(name) = path.file_name() {
            if name == OsStr::new("class.bs") {
                println!("Skipping test, since instance calling is not supported in checker workflows...");
                println!("{}\n\n", (0..80).map(|_| '-').collect::<String>());
                return;
            }
        }

        // Load the package index
        let pindex: PackageIndex = create_package_index_from(tests_path.join("packages"));
        let dindex: DataIndex = create_data_index_from(tests_path.join("data"));

        // Compile the raw source to WIR
        let wir: ast::Workflow = match compile_program(code.as_bytes(), &pindex, &dindex, &ParserOptions::bscript()) {
            CompileResult::Workflow(wir, warns) => {
                // Print warnings if any
                for w in warns {
                    w.prettyprint(path.to_string_lossy(), &code);
                }
                wir
            },
            CompileResult::Eof(err) => {
                // Print the error
                err.prettyprint(path.to_string_lossy(), &code);
                panic!("Failed to compile to WIR (see output above)");
            },
            CompileResult::Err(errs) => {
                // Print the errors
                for e in errs {
                    e.prettyprint(path.to_string_lossy(), &code);
                }
                panic!("Failed to compile to WIR (see output above)");
            },

            _ => {
                unreachable!();
            },
        };

        // Next, compile to the checker's workflow
        let mut wf: Workflow = match wir.try_into() {
            Ok(wf) => wf,
            Err(err) => {
                panic!("Failed to compile WIR to CheckerWorkflow: {err}");
            },
        };

        // Slide in that optimization
        wf.optimize();

        // Now print the file for prettyness
        println!("{}", wf.visualize());
        println!("{}\n\n", (0..80).map(|_| '-').collect::<String>());
    });
}
