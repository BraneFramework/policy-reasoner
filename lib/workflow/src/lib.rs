//  LIB.rs
//    by Lut99
//
//  Created:
//    08 Oct 2024, 16:16:26
//  Last edited:
//    08 Oct 2024, 19:19:27
//  Auto updated?
//    Yes
//
//  Description:
//!   Defines the workflow representation used internally by the checker.
//

// Declare modules
mod optimize;
pub mod visitor;
#[cfg(feature = "visualize")]
pub mod visualize;

// Imports
use enum_debug::EnumDebug;
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};


/***** AUXILLARY DATA *****/
/// Defines a representation of a dataset.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Deserialize, Serialize))]
pub struct Dataset {
    /// Some identifier of the dataset.
    pub id: String,
}

/// Represents a user/site that can compute, store data, do neither or do both.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Deserialize, Serialize))]
pub struct Entity {
    /// Some identifier of this domain.
    pub id: String,
}

/// Represents a "tag" and everything we need to know.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Deserialize, Serialize))]
pub struct Metadata {
    /// The arbitrary data embedded as metadata.
    pub tag: String,
    /// The signature verifying this metadata. If present, it's given as a pair of the person signing it and their signature.
    pub signature: Option<(Entity, String)>,
}





/***** LIBRARY *****/
/// Defines the workflow's toplevel view.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Deserialize, Serialize))]
pub struct Workflow {
    /// The identifier of this workflow as a whole.
    pub id:    String,
    /// Defines the first node in the workflow.
    pub start: Elem,

    /// The user instigating this workflow (and thus getting the result, if any).
    pub user:      Entity,
    /// The metadata associated with this workflow as a whole.
    pub metadata:  Vec<Metadata>,
    /// The signature verifying this workflow. If present, it's given as a pair of the person signing it and their signature.
    pub signature: Option<(Entity, String)>,
}

/// Defines an element in the graph. This is either a _Node_, which defines a task execution, or an _Edge_, which defines how next tasks may be reached.
#[derive(Clone, Debug, EnumDebug)]
#[cfg_attr(feature = "serde", derive(Deserialize, Serialize))]
pub enum Elem {
    // Nodes
    /// Defines a task that is executed, accessing and potentially producing data.
    Call(ElemCall),

    // Edges
    /// Defines an edge that connects to multiple next graph-branches of which only _one_ must be taken. Note that, because we don't include dynamic control flow information, we don't know _which_ will be taken.
    Branch(ElemBranch),
    /// Defines an edge that connects to multiple next graph-branches of which _all_ must be taken _concurrently_.
    Parallel(ElemParallel),
    /// Defines an edge that repeats a particular branch an unknown amount of times.
    Loop(ElemLoop),

    // Terminators
    /// Defines that the next element to execute is given by the parent `next`-field.
    ///
    /// This occurs at the end of a [`Elem::Branch`]'s branch, for example.
    Next,
    /// Defines that no more execution takes place.
    Stop,
}
impl From<ElemCall> for Elem {
    #[inline]
    fn from(value: ElemCall) -> Self { Self::Call(value) }
}
impl From<ElemBranch> for Elem {
    #[inline]
    fn from(value: ElemBranch) -> Self { Self::Branch(value) }
}
impl From<ElemParallel> for Elem {
    #[inline]
    fn from(value: ElemParallel) -> Self { Self::Parallel(value) }
}
impl From<ElemLoop> for Elem {
    #[inline]
    fn from(value: ElemLoop) -> Self { Self::Loop(value) }
}



/// Defines a task node in the graph consisting of [`Elem`]s, which defines data access.
///
/// Yeah so basically represents a task execution, with all checker-relevant information.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Deserialize, Serialize))]
pub struct ElemCall {
    /// Some identifier for this call specifically.
    pub id:     String,
    /// Some identifier for the task that is executed in this call.
    pub task:   String,
    /// Any input datasets used by the task.
    ///
    /// Note that this denotes a set of **possible** input sets. Zero or more of these may actually be used at runtime.
    pub input:  Vec<Dataset>,
    /// If there are outputs produced by this task, this names it.
    pub output: Vec<Dataset>,

    /// The location where the task is planned to be executed, if any.
    pub at: Option<Entity>,

    /// The list of metadata belonging to this task. Note: may need to be populated by the checker!
    pub metadata: Vec<Metadata>,
    /// The next graph element that this task connects to.
    pub next:     Box<Elem>,
}



/// Defines a branching connection between graph [`Elem`]ents.
///
/// Or rather, defines a linear connection between two nodes, with a set of branches in between them.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Deserialize, Serialize))]
pub struct ElemBranch {
    /// The branches of which one _must_ be taken, but we don't know which one.
    pub branches: Vec<Elem>,
    /// The next graph element after all the branches.
    pub next:     Box<Elem>,
}

/// Defines a parallel connection between graph [`Elem`]ents.
///
/// Is like a [branch](ElemBranch), except that _all_ branches are taken _concurrently_ instead of only one.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Deserialize, Serialize))]
pub struct ElemParallel {
    /// The branches, _all_ of which but be taken _concurrently_.
    pub branches: Vec<Elem>,
    /// The next graph element after all the branches.
    pub next:     Box<Elem>,
}

/// Defines a looping connection between graph [`Elem`]ents.
///
/// Simply defines a branch that is taken repeatedly. Any condition that was there is embedded in the branching part, since that's how the branch is dynamically taken and we can't know how often any of them is taken anyway.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Deserialize, Serialize))]
pub struct ElemLoop {
    /// The body (and embedded condition) of the loop.
    pub body: Box<Elem>,
    /// The next graph element that this parallel edge connects to.
    pub next: Box<Elem>,
}
