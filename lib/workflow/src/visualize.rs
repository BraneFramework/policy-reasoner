//  VISUALIZE.rs
//    by Lut99
//
//  Created:
//    31 Oct 2023, 14:30:00
//  Last edited:
//    13 Dec 2023, 08:44:28
//  Auto updated?
//    Yes
//
//  Description:
//!   Implements a simple traversal over the [`Workflow`] to print it
//!   neatly to some writer.
//

use std::fmt::{Display, Formatter, Result as FResult};

use super::spec::{Elem, ElemBranch, ElemCommit, ElemLoop, ElemParallel, ElemTask, Workflow};

/***** HELPER MACROS *****/
/// Prints a given iterator somewhat nicely to a string.
macro_rules! write_iter {
    ($iter:expr, $conn:literal) => {{
        let mut iter = $iter.peekable();
        format!(
            "{}",
            if let Some(first) = iter.next() {
                if iter.peek().is_some() {
                    format!(concat!("{}", $conn, "{}"), first, iter.collect::<Vec<String>>().join($conn))
                } else {
                    format!("{}", first)
                }
            } else {
                String::from("<none>")
            }
        )
    }};
}

/***** HELPER FUNCTIONS *****/
/// Writes an [`Elem`] to the given formatter.
///
/// # Arguments
/// - `funcs`: A map of function IDs to metadata.
/// - `f`: The [`Formatter`] to write to.
/// - `elem`: The [`Elem`] to write.
/// - `prefix`: Some prefix to write before every line.
///
/// # Errors
/// This function only errors if we failed to write to the given `f`.
fn print_elem(f: &mut Formatter, elem: &Elem, prefix: &dyn Display) -> FResult {
    // Print the element
    match elem {
        Elem::Task(ElemTask { id, name, package, version, input, output, location, metadata, next }) => {
            writeln!(f, "{prefix}task")?;
            writeln!(f, "{prefix}  - id : {id}")?;
            writeln!(f, "{prefix}")?;
            writeln!(f, "{prefix}  - name    : {name}")?;
            writeln!(f, "{prefix}  - package : {package}")?;
            writeln!(f, "{prefix}  - version : {version}")?;
            writeln!(f, "{prefix}")?;
            writeln!(f, "{prefix}  - input  : {}", write_iter!(input.iter().map(|data| format!("'{}'", data.name)), " or "))?;
            writeln!(
                f,
                "{}  - output : {}",
                prefix,
                if let Some(output) = &output { format!("'{}'", output.name.as_str()) } else { "<none>".into() }
            )?;
            writeln!(f, "{prefix}")?;
            writeln!(f, "{prefix}  - location  : {}", if let Some(location) = &location { location.as_str() } else { "<unplanned>" })?;
            writeln!(
                f,
                "{}  - metadata  : {}",
                prefix,
                write_iter!(
                    metadata.iter().map(|metadata| format!(
                        "#{}.{}{}",
                        metadata.owner,
                        metadata.tag,
                        if let Some((assigner, signature)) = &metadata.signature { format!("#{assigner}:{signature}") } else { String::new() }
                    )),
                    ", "
                )
            )?;

            // Do next
            print_elem(f, next, prefix)
        },

        Elem::Branch(ElemBranch { branches, next }) => {
            writeln!(f, "{prefix}branch")?;

            // Write the branches
            for (i, branch) in branches.iter().enumerate() {
                writeln!(f, "{prefix}{}<branch{}>", Indent(4), i)?;
                print_elem(f, branch, &Pair(prefix, Indent(8)))?;
                writeln!(f, "{prefix}")?;
            }

            // Do next
            print_elem(f, next, prefix)
        },
        Elem::Parallel(ElemParallel { branches, merge, next }) => {
            writeln!(f, "{prefix}parallel")?;
            writeln!(f, "{prefix}  - merge strategy : {merge:?}")?;

            // Write the branches
            for (i, branch) in branches.iter().enumerate() {
                writeln!(f, "{prefix}{}<branch{}>", Indent(4), i)?;
                print_elem(f, branch, &Pair(prefix, Indent(8)))?;
                writeln!(f, "{prefix}")?;
            }

            // Do next
            print_elem(f, next, prefix)
        },
        Elem::Loop(ElemLoop { body, next }) => {
            writeln!(f, "{prefix}loop")?;
            writeln!(f, "{}<repeated>", Pair(prefix, Indent(4)))?;
            print_elem(f, body, &Pair(prefix, Indent(8)))?;
            writeln!(f)?;

            // Do next
            print_elem(f, next, prefix)
        },
        Elem::Commit(ElemCommit { id, data_name, location, input, next }) => {
            writeln!(f, "{prefix}commit <{} as '{}'>", write_iter!(input.iter().map(|data| format!("'{}'", data.name)), " or "), data_name)?;
            writeln!(f, "{prefix}  - id   : {id}")?;
            for i in input {
                if let Some(from) = &i.from {
                    writeln!(f, "{prefix}  - from : '{}' <- '{}'", i.name, from)?;
                }
            }
            if let Some(location) = location {
                writeln!(f, "{prefix}  - to   : {location}")?;
            }

            // Do next
            print_elem(f, next, prefix)
        },

        Elem::Next => {
            writeln!(f, "{}next", prefix)
        },
        Elem::Stop(returns) => {
            writeln!(
                f,
                "{}stop{}",
                prefix,
                if !returns.is_empty() {
                    format!(" <returns {}>", write_iter!(returns.iter().map(|data| format!("'{}'", data.name)), " or "))
                } else {
                    String::new()
                }
            )
        },
    }
}

/***** HELPERS *****/
/// Writes two display things successively.
struct Pair<D1, D2>(D1, D2);
impl<D1: Display, D2: Display> Display for Pair<D1, D2> {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FResult { write!(f, "{}{}", self.0, self.1) }
}

/// Generates indentation of the asked size.
struct Indent(usize);
impl Display for Indent {
    fn fmt(&self, f: &mut Formatter<'_>) -> FResult {
        for _ in 0..self.0 {
            write!(f, " ")?;
        }
        Ok(())
    }
}

/***** FORMATTERS *****/
/// Capable of printing the [`Workflow`] to some writer.
#[derive(Debug)]
pub struct WorkflowFormatter<'w> {
    /// The workflow to format.
    wf: &'w Workflow,
}
impl Display for WorkflowFormatter<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FResult {
        // Print some nice header thingy
        writeln!(f, "Workflow [")?;

        // Print global metadata
        if !self.wf.metadata.is_empty() {
            writeln!(
                f,
                "{}{}",
                Indent(4),
                write_iter!(
                    self.wf.metadata.iter().map(|metadata| format!(
                        "#{}.{}{}",
                        metadata.owner,
                        metadata.tag,
                        if let Some((assigner, signature)) = &metadata.signature { format!("#{assigner}:{signature}") } else { String::new() }
                    )),
                    ", "
                )
            )?;
            writeln!(f)?;
        }

        // Alright print the main elements
        print_elem(f, &self.wf.start, &Indent(4))?;

        // Finish with the end bracket
        write!(f, "]")
    }
}

impl Workflow {
    /// Returns a nice formatter that visualizes the workflow more easily understandable than its [`Debug`](std::fmt::Debug)-implementation.
    ///
    /// # Returns
    /// A new [`WorkflowFormatter`] that can visualize the workflow when its [`Display`]-implementation is called.
    #[inline]
    pub fn visualize(&self) -> WorkflowFormatter { WorkflowFormatter { wf: self } }
}
