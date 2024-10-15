//  WORKFLOW.rs
//    by Lut99
//
//  Created:
//    11 Oct 2024, 16:54:04
//  Last edited:
//    14 Oct 2024, 11:58:50
//  Auto updated?
//    Yes
//
//  Description:
//!   Deals with analysing the reasoner's "generic" [`Workflow`] AST.
//

use std::convert::Infallible;
use std::sync::LazyLock;

use tracing::{debug, span, Level};
use workflow::visitor::Visitor;
use workflow::{Dataset, ElemCall, Entity, Workflow};


/***** CONSTANTS *****/
/// Special location that indicates to the policy that a task was unplanned.
pub static UNSPECIFIED_LOCATION: LazyLock<Entity> = LazyLock::new(|| Entity { id: "<unspecified>".into() });





/***** VISITORS *****/
/// Visits a [`Workflow`] in order to find all datasets used.
struct DatasetCollector<'w> {
    read_sets:    Vec<(&'w Entity, &'w Dataset)>,
    write_sets:   Vec<(&'w Entity, &'w Dataset)>,
    execute_sets: Vec<(&'w Entity, &'w Dataset)>,
}
impl<'w> Default for DatasetCollector<'w> {
    #[inline]
    fn default() -> Self { Self { read_sets: Default::default(), write_sets: Default::default(), execute_sets: Default::default() } }
}
impl<'w> Visitor<'w> for DatasetCollector<'w> {
    type Error = Infallible;

    // fn visit_task(&mut self, task: &workflow::ElemTask) {
    //     // FIXME: Location is not currently sent as part of the workflow validation request,
    //     // this makes this not really possible to do now. To ensure the code is working
    //     // however, we will for the mean time assume the location

    //     let location = task.location.clone().unwrap_or_else(|| String::from(ASSUMED_LOCATION));
    //     if let Some(output) = &task.output {
    //         self.read_sets.push((location.clone(), output.clone()));
    //     }
    // }

    // fn visit_commit(&mut self, commit: &workflow::ElemCommit) {
    //     let location = commit.location.clone().unwrap_or_else(|| String::from(ASSUMED_LOCATION));
    //     self.read_sets.extend(repeat(location.clone()).zip(commit.input.iter().cloned()));

    //     // TODO: Maybe create a dedicated enum type for this e.g. NewDataset for datasets that will be
    //     // created, might fail if one already exists.
    //     let location = commit.location.clone().unwrap_or_else(|| String::from(ASSUMED_LOCATION));
    //     self.write_sets.push((location.clone(), Dataset { id: commit.data_name.clone() }));
    // }

    // // TODO: We do not really have a location for this one right now, we should figure out how to
    // // interpret this
    // fn visit_stop(&mut self, stop_sets: &HashSet<Dataset>) {
    //     let location = String::from(ASSUMED_LOCATION);
    //     self.write_sets.extend(repeat(location).zip(stop_sets.iter().cloned()));
    // }

    fn visit_call(&mut self, elem: &'w ElemCall) -> Result<(), Self::Error> {
        // We take a more simplified view on dataset reading/writing.

        // We consider a task's inputs as READING. Any task's outputs are WRITING.
        // Commit collapses to this behaviour. Similarly, any special "identity" task that implies
        // the read at the end of a workflow also follows this pattern.
        // Another different from before: we rename the unknown location to "<unspecified>" to
        // be able to write policies explicitly for this use-case.

        let location: &'w Entity = elem.at.as_ref().unwrap_or_else(|| LazyLock::force(&UNSPECIFIED_LOCATION));
        self.read_sets.extend(elem.input.iter().map(|d| (location, d)));
        self.write_sets.extend(elem.output.iter().map(|d| (location, d)));
        Ok(())
    }
}





/***** LIBRARY *****/
/// The datasets accessed and/or modified in a workflow. These are grouped by file permission type. For creating this
/// struct see: [`find_datasets_in_workflow`].
pub struct WorkflowDatasets<'w> {
    pub read_sets:    Vec<(&'w Entity, &'w Dataset)>,
    pub write_sets:   Vec<(&'w Entity, &'w Dataset)>,
    pub execute_sets: Vec<(&'w Entity, &'w Dataset)>,
}
impl<'w> From<&'w Workflow> for WorkflowDatasets<'w> {
    #[inline]
    fn from(value: &'w Workflow) -> Self {
        let _span = span!(Level::INFO, "PosixReasonerConnector::find_datasets_in_workflow", workflow = value.id).entered();
        debug!("Walking the workflow in order to find datasets. Starting with {:?}", &value.start);

        let mut visitor = DatasetCollector::default();
        value.visit(&mut visitor);

        WorkflowDatasets { read_sets: visitor.read_sets, write_sets: visitor.write_sets, execute_sets: visitor.execute_sets }
    }
}
