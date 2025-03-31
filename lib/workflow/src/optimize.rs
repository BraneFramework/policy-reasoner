//  OPTIMIZE.rs
//    by Lut99
//
//  Created:
//    31 Oct 2023, 15:44:51
//  Last edited:
//    02 Nov 2023, 14:43:47
//  Auto updated?
//    Yes
//
//  Description:
//!   Optimizes a [`Workflow`] by aggregating elements that we can
//!   aggregate.
//

use std::collections::HashSet;

use log::debug;
use transform::Transform as _;

use super::spec::{Elem, ElemBranch, Workflow};

/***** HELPER FUNCTIONS *****/
/// Attempts to optimize the given branch of [`Elem`]s.
///
/// # Arguments
/// - `elem`: The [`Elem`] to optimize.
///
/// # Returns
/// Whether or not an optimization occurred. This can be used to saturate them while possible.
fn optimize_elem(elem: Elem) -> (bool, Elem) {
    // Match on the element
    match elem {
        Elem::Task(task) => (false, Elem::Task(task)),

        Elem::Branch(mut branch) => {
            // Recurse into all branches with transpose to be able to remove or merge them
            let mut changed: bool = false;
            branch.branches = branch
                .branches
                .drain(..)
                .transform(|mut b| {
                    // Optimize the branch first
                    let (b_changed, new_b) = optimize_elem(b);
                    changed |= b_changed;
                    b = new_b;

                    // Now see if we can do any branch-specific optimizations
                    match b {
                        // If the only thing in the branch is a next, then don't consider it anymore
                        Elem::Next => {
                            debug!("Applied optimization: empty branch pruning");
                            changed = true;
                            vec![]
                        },

                        // If the branch only exists of branches, then we can collapse them into ourselves
                        Elem::Branch(nested_branch) => {
                            // Assert it only exists of branches
                            let mut next_branch: &ElemBranch = &nested_branch;
                            let mut only_branches: bool = true;
                            loop {
                                match &*next_branch.next {
                                    Elem::Branch(nb) => {
                                        next_branch = nb;
                                    },
                                    Elem::Next => {
                                        break;
                                    },
                                    _ => {
                                        only_branches = false;
                                        break;
                                    },
                                }
                            }

                            // If so, iterate again, aggregating all branches
                            if only_branches {
                                let mut branches: Vec<Elem> = nested_branch.branches;
                                let mut next_branch: Box<Elem> = nested_branch.next;
                                while let Elem::Branch(nb) = *next_branch {
                                    branches.extend(nb.branches);
                                    next_branch = nb.next;
                                }
                                debug!("Applied optimization: branch aggregation");
                                changed = true;
                                branches
                            } else {
                                vec![Elem::Branch(nested_branch)]
                            }
                        },

                        // Nothing to do for the rest
                        b => vec![b],
                    }
                })
                .collect();

            // If the branches are empty, then replace the next with the branch
            if branch.branches.is_empty() {
                let (_, next) = optimize_elem(*branch.next);
                (true, next)
            } else {
                // Now optimize the res of the program as normal
                let (next_changed, next) = optimize_elem(*branch.next);
                branch.next = Box::new(next);
                (changed | next_changed, Elem::Branch(branch))
            }
        },
        Elem::Parallel(parallel) => (false, Elem::Parallel(parallel)),
        Elem::Loop(l) => (false, Elem::Loop(l)),
        Elem::Commit(commit) => (false, Elem::Commit(commit)),

        Elem::Next => (false, Elem::Next),
        Elem::Stop(returns) => (false, Elem::Stop(returns)),
    }
}

/***** LIBRARY *****/
impl Workflow {
    /// Optimizes the workflow graph by pruning elements which do task-independent things (like branching without tasks) and aggregates aggregatable edges.
    pub fn optimize(&mut self) {
        let Self { start, .. } = self;

        // Get the start out of self
        let mut graph: Elem = Elem::Stop(HashSet::new());
        std::mem::swap(&mut graph, start);

        // Decide which functions can be discarded
        /* TODO */

        // Analyze the individual edges first
        loop {
            let (changed, new_graph) = optimize_elem(graph);
            graph = new_graph;
            if !changed {
                break;
            }
        }

        // Restore the graph
        std::mem::swap(start, &mut graph);
    }
}
