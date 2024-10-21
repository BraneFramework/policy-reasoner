//  OPTIMIZE.rs
//    by Lut99
//
//  Created:
//    08 Oct 2024, 17:34:14
//  Last edited:
//    05 May 2025, 12:04:21
//  Auto updated?
//    Yes
//
//  Description:
//!   Optimizes a [`Workflow`] in various ways.
//

use std::convert::Infallible;

use crate::visitor::VisitorOwned;
use crate::{Elem, ElemBranch, Workflow};


/***** TESTS *****/
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ElemCall, Entity};


    /// Generates a workflow with minimal info
    #[inline]
    fn gen_wf(id: impl Into<String>, start: impl Into<Elem>) -> Workflow {
        Workflow { id: id.into(), start: start.into(), user: Some(Entity { id: "amy".into() }), metadata: vec![], signature: None }
    }

    /// Generates a branch.
    #[inline]
    fn gen_branch(branches: impl IntoIterator<Item = Elem>, next: Elem) -> Elem {
        Elem::Branch(ElemBranch { branches: branches.into_iter().collect(), next: Box::new(next) })
    }

    /// Generates a call to a specific package, nothing else.
    #[inline]
    fn gen_void_call(id: impl Into<String>, task: impl Into<String>, next: Elem) -> Elem {
        Elem::Call(ElemCall { id: id.into(), task: task.into(), input: vec![], output: vec![], at: None, metadata: vec![], next: Box::new(next) })
    }


    /// Checks if two workflows have the same structure.
    #[inline]
    fn compares(left: &Workflow, right: &Workflow) -> bool {
        fn cmp_edge(left: &Elem, right: &Elem) -> bool {
            match (left, right) {
                (Elem::Call(l), Elem::Call(r)) => l.task == r.task && cmp_edge(&l.next, &r.next),

                (Elem::Branch(l), Elem::Branch(r)) => {
                    l.branches.len() == r.branches.len()
                        && l.branches.iter().zip(r.branches.iter()).all(|(l, r)| cmp_edge(l, r))
                        && cmp_edge(&l.next, &r.next)
                },
                (Elem::Parallel(l), Elem::Parallel(r)) => {
                    l.branches.len() == r.branches.len()
                        && l.branches.iter().zip(r.branches.iter()).all(|(l, r)| cmp_edge(l, r))
                        && cmp_edge(&l.next, &r.next)
                },
                (Elem::Loop(l), Elem::Loop(r)) => cmp_edge(&l.body, &r.body) && cmp_edge(&l.next, &r.next),

                (Elem::Next, Elem::Next) => true,
                (Elem::Stop, Elem::Stop) => true,

                _ => false,
            }
        }

        cmp_edge(&left.start, &right.start)
    }



    /// Tests whether branches are flattened.
    #[test]
    fn test_branch_flattener() {
        // Case 1
        let mut pred: Workflow = gen_wf(
            "Prediction",
            gen_branch(
                [
                    gen_branch([gen_void_call("foo", "Foo", Elem::Next)], Elem::Next),
                    gen_branch([gen_void_call("bar", "Bar", Elem::Next), gen_void_call("baz", "Baz", Elem::Next)], Elem::Next),
                ],
                Elem::Stop,
            ),
        );
        pred.optimize();
        assert!(compares(
            &pred,
            &gen_wf(
                "Truth",
                gen_branch(
                    [gen_void_call("foo", "Foo", Elem::Next), gen_void_call("bar", "Bar", Elem::Next), gen_void_call("baz", "Baz", Elem::Next)],
                    Elem::Stop
                )
            )
        ));
    }

    /// Tests whether dead branches are pruned.
    #[test]
    fn test_dead_branch_pruner() {
        // Case 1
        let mut pred: Workflow = gen_wf("Prediction", gen_branch([gen_void_call("foo", "Foo", Elem::Next), Elem::Next], Elem::Stop));
        pred.optimize();
        assert!(compares(&pred, &gen_wf("Truth", gen_branch([gen_void_call("foo", "Foo", Elem::Next),], Elem::Stop))));
    }

    /// Tests whether empty branches are removed.
    #[test]
    fn test_empty_branch_remover() {
        // Case 1
        let mut pred: Workflow = gen_wf("Prediction", gen_branch([], Elem::Stop));
        pred.optimize();
        assert!(compares(&pred, &gen_wf("Truth", Elem::Stop)));
    }
}





/***** HELPERS *****/
// Interface
/// Defines what an optimizer looks like in the abstract.
trait Optimizer: VisitorOwned {
    /// Applies the optimizer to the given workflow.
    ///
    /// # Returns
    /// Whether anything has been optimized.
    fn optimize(wf: &mut Workflow) -> bool
    where
        Self: Default + VisitorOwned<Error = Infallible>,
    {
        let mut opt: Self = Self::default();
        wf.visit_owned(&mut opt).unwrap();
        opt.has_optimized()
    }

    /// Returns whether this optimizer has done anything.
    fn has_optimized(&self) -> bool;
}



// Branches
/// Optimizes the workflow graph by collapsing a branch that only has branches as elements.
struct BranchFlattener {
    /// Keeps track of whether this optimizer has done anything.
    ///
    /// Used to saturate the process.
    optimized: bool,
}
impl Default for BranchFlattener {
    #[inline]
    fn default() -> Self { Self { optimized: false } }
}
impl Optimizer for BranchFlattener {
    #[inline]
    fn has_optimized(&self) -> bool { self.optimized }
}
impl VisitorOwned for BranchFlattener {
    type Error = Infallible;

    fn visit_branch(&mut self, mut elem: ElemBranch) -> Result<Elem, Self::Error> {
        // Investigate whether all nested branches are, themselves, branches that terminate where
        // this branch terminates.
        for b in &elem.branches {
            // First, verify it's a branch
            if let Elem::Branch(b) = b {
                // Then verify nothing follows it
                if !matches!(*b.next, Elem::Next) {
                    return Ok(Elem::Branch(elem));
                }
            } else {
                return Ok(Elem::Branch(elem));
            }
        }

        // OK, now we collapse all branches into this one
        elem.branches = elem.branches.drain(..).flat_map(|b| if let Elem::Branch(b) = b { b.branches.into_iter() } else { unreachable!() }).collect();
        self.optimized = true;

        // Now continue by calling the newly set branches
        for b in &mut elem.branches {
            self.visit_mut(b)?;
        }
        self.visit_mut(&mut elem.next)?;

        // OK
        Ok(Elem::Branch(elem))
    }
}

/// Optimizes the workflow graph by pruning empty branches.
struct DeadBranchPruner {
    /// Keeps track of whether this optimizer has done anything.
    ///
    /// Used to saturate the process.
    optimized: bool,
}
impl Default for DeadBranchPruner {
    #[inline]
    fn default() -> Self { Self { optimized: false } }
}
impl Optimizer for DeadBranchPruner {
    #[inline]
    fn has_optimized(&self) -> bool { self.optimized }
}
impl VisitorOwned for DeadBranchPruner {
    type Error = Infallible;

    fn visit_branch(&mut self, mut elem: ElemBranch) -> Result<Elem, Self::Error> {
        // Investigate which branches to keep (we discard any that are immediately terminating)
        let old_len: usize = elem.branches.len();
        elem.branches = elem.branches.drain(..).filter(|e| !matches!(e, Elem::Next)).collect();
        self.optimized |= elem.branches.len() != old_len;

        // Now continue by calling all the remaining nested branches and then the next node
        for b in &mut elem.branches {
            self.visit_mut(b)?;
        }
        self.visit_mut(&mut elem.next)?;

        // OK
        Ok(Elem::Branch(elem))
    }
}

/// Optimizes the workflow graph by pruning empty [`Elem::Branch`]es.
struct EmptyBranchRemover {
    /// Keeps track of whether this optimizer has done anything.
    ///
    /// Used to saturate the process.
    optimized: bool,
}
impl Default for EmptyBranchRemover {
    #[inline]
    fn default() -> Self { Self { optimized: false } }
}
impl Optimizer for EmptyBranchRemover {
    #[inline]
    fn has_optimized(&self) -> bool { self.optimized }
}
impl VisitorOwned for EmptyBranchRemover {
    type Error = Infallible;

    fn visit_branch(&mut self, mut elem: ElemBranch) -> Result<Elem, Self::Error> {
        // If there are no more branches here, then replace with next
        if elem.branches.is_empty() {
            self.optimized = true;
            self.visit_mut(&mut elem.next)?;
            Ok(*elem.next)
        } else {
            for b in &mut elem.branches {
                self.visit_mut(b)?;
            }
            self.visit_mut(&mut elem.next)?;
            Ok(Elem::Branch(elem))
        }
    }
}





/***** LIBRARY *****/
impl Workflow {
    /// Optimizes the workflow graph by pruning elements which do task-independent things (like branching without tasks) and aggregates aggregatable edges.
    pub fn optimize(&mut self) {
        // Optimize while anything's being optimized
        let mut saturated: bool = false;
        while !saturated {
            // Apply the optimizations
            saturated = !(BranchFlattener::optimize(self) | DeadBranchPruner::optimize(self) | EmptyBranchRemover::optimize(self));
        }
    }
}
