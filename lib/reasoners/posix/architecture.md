## Design

As of now, the assumption is taken that it does not matter for this reasoner which type of request comes in, as we only
look at the data usage in the [Workflow].

As one of these requests comes in, the provided [Workflow] is parsed to collect the referenced datasets
[WorkflowDatasets] and all data accesses in the workflow are gathered and associated with an access type of either
read, write, or execute (execute is currently unused as no usage was found).

From this point, we iterate over all the different datasets and associated requests/required permissions. For each
[Dataset] we look up the path in the [DataIndex]. Now that we have the path and the requested permissions, we can check
if the user in the mapping has access to this dataset.
