//  EFLINT.rs
//    by Lut99
//
//  Created:
//    08 Nov 2023, 14:44:31
//  Last edited:
//    12 Jun 2024, 17:39:40
//  Auto updated?
//    Yes
//
//  Description:
//!   Defines a compiler of the Checker Workflow to the eFLINT JSON
//!   Specification.
//

use std::collections::{HashMap, HashSet};

use eflint_json::spec::{ConstructorInput, Expression, ExpressionConstructorApp, ExpressionPrimitive, Phrase, PhraseCreate};
use enum_debug::EnumDebug as _;
use log::{trace, warn};
use rand::Rng as _;
use rand::distr::Alphanumeric;

use crate::spec::{Dataset, Elem, ElemBranch, ElemCommit, ElemLoop, ElemParallel, ElemTask, Metadata, User, Workflow};

/***** HELPER MACROS *****/
/// Shorthand for creating an eFLINT JSON Specification true postulation.
macro_rules! create {
    ($inst:expr) => {
        Phrase::Create(PhraseCreate { operand: $inst })
    };
}

/// Shorthand for creating an eFLINT JSON Specification constructor application.
macro_rules! constr_app {
    ($id:expr $(, $args:expr)* $(,)?) => {
        Expression::ConstructorApp(ExpressionConstructorApp {
            identifier: ($id).into(),
            operands:   ConstructorInput::ArraySyntax(vec![ $($args),* ]),
        })
    };
}

/// Shorthand for creating an eFLINT JSON Specification string literal.
macro_rules! str_lit {
    ($val:expr) => {
        Expression::Primitive(ExpressionPrimitive::String(($val).into()))
    };
}

/***** HELPER FUNCTIONS *****/
/// Simple traversal that names all [`ElemLoop`]s.
///
/// # Arguments
/// - `elem`: The graph [`Elem`]ent to analyse.
/// - `wf_id`: The identifier of the workflow to use for new loop IDs.
/// - `loops`: A map of pointers to their IDs.
fn name_loops(mut elem: &Elem, wf_id: &str, loops: &mut HashMap<*const ElemLoop, String>) {
    // Note we're doing a combination of actual recursion and looping, to minimize stack usage
    loop {
        match elem {
            Elem::Task(ElemTask { id: _, name: _, package: _, version: _, input: _, output: _, location: _, metadata: _, next }) => elem = next,
            Elem::Commit(ElemCommit { id: _, data_name: _, location: _, input: _, next }) => elem = next,

            Elem::Branch(ElemBranch { branches, next }) => {
                for branch in branches {
                    name_loops(branch, wf_id, loops);
                }
                elem = next;
            },
            Elem::Parallel(ElemParallel { merge: _, branches, next }) => {
                for branch in branches {
                    name_loops(branch, wf_id, loops);
                }
                elem = next;
            },
            Elem::Loop(l) => {
                let ElemLoop { body, next } = l;

                // Generate a name for this loop
                loops.insert(
                    l as *const ElemLoop,
                    format!("{wf_id}-{}-loop", rand::rng().sample_iter(Alphanumeric).take(4).map(char::from).collect::<String>()),
                );

                // Continue
                name_loops(body, wf_id, loops);
                elem = next;
            },

            Elem::Stop(_) => return,
            Elem::Next => return,
        }
    }
}

/// Analyses the given loop's body branch of the graph to find various details.
///
/// # Arguments
/// - `elem`: The graph [`Elem`]ent to analyse.
/// - `loop_names`: A map of [`ElemLoop`]s to names we computed beforehand.
/// - `first`: The first node(s) (node, commit or loop) in the subgraph.
/// - `last`: The last node(s) (node, commit or loop) in the subgraph.
///
/// If no nodes are within this body, [`None`] is returned instead.
fn analyse_loop_body(
    mut elem: &Elem,
    loop_names: &HashMap<*const ElemLoop, String>,
    first: &mut Vec<(String, HashSet<Dataset>)>,
    last: &mut HashSet<Dataset>,
) {
    // Note we're doing a combination of actual recursion and looping, to minimize stack usage
    loop {
        match elem {
            Elem::Task(ElemTask { id, name: _, package: _, version: _, input, output, location: _, metadata: _, next }) => {
                // Add it if it's the first one we encounter
                if first.is_empty() {
                    *first = vec![(id.clone(), input.iter().cloned().collect())];
                }
                // Always add as the last one
                *last = output.iter().cloned().collect();

                // Continue with iteration
                elem = next;
            },
            Elem::Commit(ElemCommit { id, data_name, location, input, next }) => {
                // Add it if it's the first one we encounter
                if first.is_empty() {
                    *first = vec![(id.clone(), input.iter().cloned().collect())];
                }
                // Always add as the last one
                *last = HashSet::from([Dataset { name: data_name.clone(), from: location.clone() }]);

                // Continue with iteration
                elem = next;
            },

            Elem::Branch(ElemBranch { branches, next }) | Elem::Parallel(ElemParallel { merge: _, branches, next }) => {
                // Aggregate the inputs & outputs of the branches
                let mut branch_firsts: Vec<(String, HashSet<Dataset>)> = Vec::new();
                let mut branch_lasts: HashSet<Dataset> = HashSet::new();
                for branch in branches {
                    let mut branch_first: Vec<(String, HashSet<Dataset>)> = Vec::new();
                    let mut branch_last: HashSet<Dataset> = HashSet::new();
                    analyse_loop_body(branch, loop_names, &mut branch_first, &mut branch_last);
                    branch_firsts.extend(branch_first);
                    branch_lasts.extend(branch_last);
                }

                // Add them to this branch' result
                if first.is_empty() {
                    *first = branch_firsts;
                }
                *last = branch_lasts;

                // Continue with iteration
                elem = next;
            },
            Elem::Loop(l) => {
                let ElemLoop { body, next } = l;

                // We recurse to find the inputs- and outputs
                let mut body_first: Vec<(String, HashSet<Dataset>)> = vec![];
                let mut body_last: HashSet<Dataset> = HashSet::new();
                analyse_loop_body(body, loop_names, &mut body_first, &mut body_last);

                // Propagate these
                if first.is_empty() {
                    // Get the loop's name
                    let id: &String =
                        loop_names.get(&(l as *const ElemLoop)).unwrap_or_else(|| panic!("Encountered loop without name after loop naming"));

                    // Set this loop as the first node, combining all the input dataset from the children
                    *first = vec![(id.clone(), body_first.into_iter().flat_map(|(_, data)| data).collect::<HashSet<Dataset>>())]
                }
                *last = body_last;

                // Continue with iteration
                elem = next;
            },

            Elem::Stop(_) => return,
            Elem::Next => return,
        }
    }
}

/// Compiles a given piece of metadata.
///
/// # Arguments
/// - `metadata`: The [`Metadata`] to compile.
/// - `phrases`: The buffer to compile to.
fn compile_metadata(metadata: &Metadata, phrases: &mut Vec<Phrase>) {
    // First, we push the tag
    // ```eflint
    // +tag(user(#metadata.owner), #metadata.tag).
    // ```
    let tag: Expression = constr_app!("tag", constr_app!("user", str_lit!(metadata.owner.clone())), str_lit!(metadata.tag.clone()));
    phrases.push(create!(tag.clone()));

    // Push the signature
    let signature: Expression = if let Some((assigner, signature)) = &metadata.signature {
        // ```eflint
        // +signature(user(#assigner), #signature).
        // ```
        constr_app!("signature", constr_app!("user", str_lit!(assigner.clone())), str_lit!(signature.clone()))
    } else {
        // Push an empty signature, to be sure that the one is in serialized metadata is still findable
        // ```eflint
        // +signature(user(""), "").
        // ```
        constr_app!("signature", constr_app!("user", str_lit!("")), str_lit!(""))
    };
    phrases.push(create!(signature.clone()));

    // Then push the metadata as a whole
    phrases.push(create!(constr_app!("metadata", tag, signature)));
}

/// Compiles the given [`Elem`] onwards to a series of eFLINT [`Phrase`]s.
///
/// # Arguments
/// - `elem`: The current [`Elem`] we're compiling.
/// - `wf_id`: The identifier/name of the workflow we're working with.
/// - `wf_user`: The identifier/name of the user who will see the workflow result.
/// - `loop_names`: A map of [`ElemLoop`]s to names we computed beforehand.
/// - `phrases`: The list of eFLINT [`Phrase`]s we're compiling to.
fn compile_eflint(mut elem: &Elem, wf_id: &str, wf_user: &User, loop_names: &HashMap<*const ElemLoop, String>, phrases: &mut Vec<Phrase>) {
    // Note we're doing a combination of actual recursion and looping, to minimize stack usage
    loop {
        trace!("Compiling {:?} to eFLINT", elem.variant());
        match elem {
            Elem::Task(ElemTask { id, name, package, version, input, output, location, metadata, next }) => {
                // Define a new task call and make it part of the workflow
                // ```eflint
                // +node(workflow(#wf_id), #id).
                // +task(node(workflow(#wf_id), #id)).
                // ```
                let node: Expression = constr_app!("node", constr_app!("workflow", str_lit!(wf_id)), str_lit!(id.clone()));
                phrases.push(create!(node.clone()));
                phrases.push(create!(constr_app!("task", node.clone())));

                // Link the code input
                // ```eflint
                // +node-input(#node, asset("#package-#version")).
                // +function(node-input(#node, asset("#package-#version")), #name).
                // ```
                let code_input: Expression =
                    constr_app!("node-input", node.clone(), constr_app!("asset", str_lit!(format!("{}-{}", package, version))));
                phrases.push(create!(code_input.clone()));
                phrases.push(create!(constr_app!("function", code_input.clone(), str_lit!(name.clone()))));

                // Add its inputs
                for i in input {
                    // Link this input to the task
                    // ```eflint
                    // +node-input(#node, asset(#i.name)).
                    // ```
                    let node_input: Expression = constr_app!("node-input", node.clone(), constr_app!("asset", str_lit!(i.name.clone())));
                    phrases.push(create!(node_input.clone()));

                    // Add where this dataset lives if we know that
                    if let Some(from) = &i.from {
                        // It's planned to be transferred from this location
                        // ```eflint
                        // +node-input-from(#node-input, domain(user(#from))).
                        // ```
                        phrases.push(create!(constr_app!(
                            "node-input-from",
                            node_input,
                            constr_app!("domain", constr_app!("user", str_lit!(from.clone())))
                        )));
                    } else if let Some(at) = location {
                        // It's present on the task's location
                        // ```eflint
                        // +node-input-from(#node-input, domain(user(#at))).
                        // ```
                        phrases.push(create!(constr_app!(
                            "node-input-from",
                            node_input,
                            constr_app!("domain", constr_app!("user", str_lit!(at.clone())))
                        )));
                    } else {
                        warn!("Encountered input dataset '{}' without transfer source in task '{}' as part of workflow '{}'", i.name, id, wf_id);
                    }
                }
                // Add the output, if any
                if let Some(o) = &output {
                    // ```eflint
                    // +node-output(#node, asset(#o.name)).
                    // ```
                    phrases.push(create!(constr_app!("node-output", node.clone(), constr_app!("asset", str_lit!(o.name.clone())))));
                }
                // Add the location of the task execution
                if let Some(at) = location {
                    // ```eflint
                    // +node-at(#node, domain(user(#at))).
                    // ```
                    phrases.push(create!(constr_app!("node-at", node.clone(), constr_app!("domain", constr_app!("user", str_lit!(at.clone()))))));
                } else {
                    warn!("Encountered unplanned task '{id}' part of workflow '{wf_id}'");
                }

                // Finally, add any task metadata
                for m in metadata {
                    // Write the metadata's children
                    compile_metadata(m, phrases);

                    // Resolve the metadata's signature
                    let (assigner, signature): (&str, &str) =
                        m.signature.as_ref().map(|(assigner, signature)| (assigner.as_str(), signature.as_str())).unwrap_or(("", ""));

                    // Write the phrase
                    // ```eflint
                    // +node-metadata(#node, metadata(tag(user(#m.owner), #m.tag), signature(user(#m.assigner), #m.signature)))).
                    // ```
                    phrases.push(create!(constr_app!(
                        "node-metadata",
                        node.clone(),
                        constr_app!(
                            "metadata",
                            constr_app!("tag", constr_app!("user", str_lit!(m.owner.clone())), str_lit!(m.tag.clone())),
                            constr_app!("signature", constr_app!("user", str_lit!(assigner)), str_lit!(signature)),
                        )
                    )));
                }

                // OK, move to the next
                elem = next;
            },
            Elem::Commit(ElemCommit { id, data_name, location, input, next }) => {
                // Add the commit task
                // ```eflint
                // +node(workflow(#wf_id), #id).
                // +commit(node(workflow(#wf_id), #id)).
                // ```
                let node: Expression = constr_app!("node", constr_app!("workflow", str_lit!(wf_id)), str_lit!(id.clone()));
                phrases.push(create!(node.clone()));
                phrases.push(create!(constr_app!("commit", node.clone())));

                // Add the commits it (possibly!) does
                for i in input {
                    // ```eflint
                    // +node-input(#node, asset(#i.name)).
                    // ```
                    let node_input: Expression = constr_app!("node-input", node.clone(), constr_app!("asset", str_lit!(i.name.clone())));
                    phrases.push(create!(node_input.clone()));

                    // Add where this dataset lives if we know that
                    if let Some(from) = &i.from {
                        // It's planned to be transferred from this location
                        // ```eflint
                        // +node-input-from(#node-input, domain(user(#from))).
                        // ```
                        phrases.push(create!(constr_app!(
                            "node-input-from",
                            node_input,
                            constr_app!("domain", constr_app!("user", str_lit!(from.clone())))
                        )));
                    } else {
                        warn!("Encountered input dataset '{}' without transfer source in commit '{}' as part of workflow '{}'", i.name, id, wf_id);
                    }
                }
                // Add the output of the node
                // ```eflint
                // +node-output(#node, asset(#data_name)).
                // +workflow-result(workflow(#wf_id), asset(#data_name)).
                // ```
                phrases.push(create!(constr_app!("node-output", node.clone(), constr_app!("asset", str_lit!(data_name.clone())))));
                phrases.push(create!(constr_app!(
                    "workflow-result",
                    constr_app!("workflow", str_lit!(wf_id)),
                    constr_app!("asset", str_lit!(data_name.clone()))
                )));

                // Add the location of this commit
                if let Some(location) = location {
                    // ```eflint
                    // +node-at(#node, domain(user(#at))).
                    // ```
                    phrases.push(create!(constr_app!("node-at", node, constr_app!("domain", constr_app!("user", str_lit!(location.clone()))))));
                }

                // Continue with the next
                elem = next;
            },

            Elem::Branch(ElemBranch { branches, next }) => {
                // Do the branches in sequence
                for branch in branches {
                    compile_eflint(branch, wf_id, wf_user, loop_names, phrases);
                }
                // Continue with the next one
                elem = next;
            },
            Elem::Parallel(ElemParallel { branches, merge: _, next }) => {
                // Do the branches in sequence
                for branch in branches {
                    compile_eflint(branch, wf_id, wf_user, loop_names, phrases);
                }
                // Continue with the next one
                elem = next;
            },
            Elem::Loop(ElemLoop { body, next }) => {
                // Serialize the body phrases first
                compile_eflint(body, wf_id, wf_user, loop_names, phrases);

                // Serialize the node
                // ```eflint
                // +node(workflow(#wf_id), #id).
                // +commit(node(workflow(#wf_id), #id)).
                // ```
                let id: String = format!("{}-{}-loop", wf_id, rand::rng().sample_iter(Alphanumeric).take(4).map(char::from).collect::<String>());
                let node: Expression = constr_app!("node", constr_app!("workflow", str_lit!(wf_id)), str_lit!(id.clone()));
                phrases.push(create!(node.clone()));
                phrases.push(create!(constr_app!("loop", node.clone())));

                // Collect the inputs & outputs of the body
                let mut first: Vec<(String, HashSet<Dataset>)> = Vec::new();
                let mut last: HashSet<Dataset> = HashSet::new();
                analyse_loop_body(body, loop_names, &mut first, &mut last);

                // Post-process the input into a list of body nodes and a list of data input
                let (bodies, inputs): (Vec<String>, Vec<HashSet<Dataset>>) = first.into_iter().unzip();
                let inputs: HashSet<Dataset> = inputs.into_iter().flatten().collect();

                // Add the loop inputs
                for input in inputs {
                    // ```eflint
                    // +node-input(#node, asset(#i.name)).
                    // ```
                    let node_input: Expression = constr_app!("node-input", node.clone(), constr_app!("asset", str_lit!(input.name.clone())));
                    phrases.push(create!(node_input.clone()));

                    // Add where this dataset lives if we know that
                    if let Some(from) = &input.from {
                        // It's planned to be transferred from this location
                        // ```eflint
                        // +node-input-from(#node-input, domain(user(#from))).
                        // ```
                        phrases.push(create!(constr_app!(
                            "node-input-from",
                            node_input,
                            constr_app!("domain", constr_app!("user", str_lit!(from.clone())))
                        )));
                    } else {
                        warn!(
                            "Encountered input dataset '{}' without transfer source in commit '{}' as part of workflow '{}'",
                            input.name, id, wf_id
                        );
                    }
                }
                // Add the loop outputs
                for output in last {
                    // ```eflint
                    // +node-output(#node, asset(#output.name)).
                    // ```
                    phrases.push(create!(constr_app!("node-output", node.clone(), constr_app!("asset", str_lit!(output.name.clone())))));
                }
                // Add the loop's bodies
                for body in bodies {
                    // ```eflint
                    // +loop-body(loop(#node), node(workflow(#wf_id), #body)).
                    // ```
                    phrases.push(create!(constr_app!(
                        "loop-body",
                        constr_app!("loop", node.clone()),
                        constr_app!("node", constr_app!("workflow", str_lit!(wf_id)), str_lit!(body))
                    )));
                }

                // Done, continue with the next one
                elem = next;
            },

            Elem::Next => return,
            Elem::Stop(results) => {
                // Mark the results as results of the workflow
                for r in results {
                    // ```eflint
                    // +workflow-result-recipient(workflow-result(workflow(#wf_id), asset(#r.name)), user(#wf_user.name)).
                    // ```
                    phrases.push(create!(constr_app!(
                        "workflow-result-recipient",
                        constr_app!("workflow-result", constr_app!("workflow", str_lit!(wf_id)), constr_app!("asset", str_lit!(r.name.clone()))),
                        constr_app!("user", str_lit!(wf_user.name.clone())),
                    )));
                }

                // Done
                return;
            },
        }
    }
}

/***** LIBRARY *****/
impl Workflow {
    /// Compiles the Workflow to a series of eFLINT phrases.
    ///
    /// Note that this only creates references to datasets, functions and users; any definition of them needs to be added separately.
    ///
    /// # Returns
    /// A series of eFLINT statements that represent this Workflow.
    pub fn to_eflint(&self) -> Vec<Phrase> {
        let mut phrases: Vec<Phrase> = vec![];

        // First, we shall name all loops
        let mut loop_names: HashMap<*const ElemLoop, String> = HashMap::new();
        name_loops(&self.start, &self.id, &mut loop_names);

        // Kick off the first phrase(s) by adding the notion of the workflow as a whole
        // ```eflint
        // +workflow(#self.id).
        // ```
        let workflow: Expression = constr_app!("workflow", str_lit!(self.id.clone()));
        phrases.push(create!(workflow.clone()));

        // Add workflow metadata
        for m in &self.metadata {
            // Write the metadata's children
            compile_metadata(m, &mut phrases);

            // Resolve the metadata's signature
            let (assigner, signature): (&str, &str) =
                m.signature.as_ref().map(|(assigner, signature)| (assigner.as_str(), signature.as_str())).unwrap_or(("", ""));

            // Write the phrase
            // ```eflint
            // +workflow-metadata(#workflow, metadata(tag(user(#m.owner), #m.tag), signature(user(#m.assigner), #m.signature)))).
            // ```
            phrases.push(create!(constr_app!(
                "workflow-metadata",
                workflow.clone(),
                constr_app!(
                    "metadata",
                    constr_app!("tag", constr_app!("user", str_lit!(m.owner.clone())), str_lit!(m.tag.clone())),
                    constr_app!("signature", constr_app!("user", str_lit!(assigner)), str_lit!(signature)),
                )
            )));
        }

        // Compile the 'flow to a list of phrases
        compile_eflint(&self.start, &self.id, &self.user, &loop_names, &mut phrases);

        // Done!
        phrases
    }
}
