POSIX Reasoner
==============

A policy reasoner implementation based on POSIX file permissions.

This documentation is aimed at developers that want to maintain or extend the POSIX reasoner. High level documentation
for users of the POSIX reasoner can be found in the [Brane user
guide](https://wiki.enablingpersonalizedinterventions.nl/user-guide/). An explanation of the POSIX policy file can also
be found there.

## Goal

This policy reasoner is meant to be easy and widely applicable. The aim is to take few assumptions and require as little
configuration as possible, allowing this reasoner to function as an easy to deploy proof of concept. This allows users
of Brane to gather experience with the abstract concept of a policy reasoner, before one has to start writing policies
themselves.

Additionally, it could function well as an initial reasoner as a user adopts Brane on their systems, since it could use
permissions already set on their current systems to infer which users have access to what data.

## Design

To describe the design of this policy reasoner, we will walk through both the setup and usage of the reasoner from a
high level point of view.

First, it checks the `DATA_INDEX` environment variable or the .env file for the location of the data index. We imagine
this points towards a mounted distributed file system like NFS. Then, it scans the directories for data index files.
From these files a [DataIndex] is created which is passed on to the [PosixReasonerConnector].

Now that the [PosixReasonerConnector] is created, it can start to handle requests. There are three types of requests:

- [Execute task](fn@PosixReasonerConnector::execute_task)
- [Access data](fn@PosixReasonerConnector::access_data_request)
- [Workflow validation](fn@PosixReasonerConnector::workflow_validation_request)

As of now, the assumption is taken that it does not matter for this reasoner which type of request comes in, as we only
look at the data usage in the [Workflow].

As one of these requests comes in, the provided [Workflow] is parsed using a [DatasetCollectorVisitor] (an
implementation of the new util trait [WorkflowVisitor]) and all data accesses in the workflow are gathered and
associated with an access type of either read, write, or execute (execute is currently unused as no usage was found).

From this point, we iterate over all the different datasets and associated requests/required permissions. For each
[Dataset] we look up the path in the [DataIndex]. Now that we have the path and the requested permissions, we can check
if the user in the mapping has access to this dataset.

### Current permission model

The current permission model is based on the POSIX file permissions. This means that we check if the user has the
required permissions on the file. This is done by checking the file permissions of the file itself, and checking if the
user is either the owner of the file, in the group of the file, or if the file is world readable. The uid and the gids
extracted from the policy are matched against the file's uid and gid. If the file is owned by the user, the owner
permissions are checked. If the file is owned by a group the user is in, the group permissions are checked. If neither
of these is true, the other permissions are checked. If the user has the required permissions, the request is approved.
If not, the request is denied : [satisfies_posix_permissions].


## State of the implementation

Right now, the POSIX policy reasoner works. One can submit workflow requests using a HTTP request, or one can use the
Policy Reasoner GUI to submit a workflow request to the policy reasoner. The policy reasoner will then evaluate if all
files are properly accessible and return a verdict.

### Limitations

We had to draw a line in the sand for the current implementation, after which we considered additional details or
features out of the scope for the current project. If more time would have been available, there would have been a fair
bit of additional features that we would have liked to have implemented.

The main origin of this line is the lack of an actual implementation inside Brane. To fully understand the implications
of the POSIX file permissions scheme, we need to start using the current implementation in a staging environment. We
should investigate the effects of multiple sites with many datasets. However, right now it is not possible (or so we
have been told) to mount the network shares to the reasoner container.

Right now, this implementation is limited by the fact that it will try to load a single data index from the `DATA_INDEX`
environment variable. However, once we start mounting multiple network shares, we imagine that it might be nice to be
able to load multiple data indices.

Another limitation is that the current implementation is not fully POSIX compliant. We still need to figure out how some
of the POSIX permission behaviours map into this emulation. E.g., right now we only check the file permissions on the
file itself, we do not check the permissions on the directory. Since we are going to be working with network shares (and
possible hard/symlinks) this becomes non-trivial, a working implementation is needed to investigate what behaviour is
desired. This is compounded by the problem that not only the user needs to be able to access the data, but also the
policy reasoner needs to reach at least the directory in which the file resides in order for the reasoner to be able to
`stat(1)` the file.

Right now, we are seemingly limited by the fact that the policy reasoner GUI does not send at which site a [Dataset] is
accessed, making it impossible to fully know which of the mappings in the policy to use. There is a location field that
could be used, but since it is always set to `None`, we opted to fall back to the assumed location as hardcoded in the
static: [`ASSUMED_LOCATION`]

## Future work

- Support file creation: This requires us to look at the permissions of the parent directory of the potential dataset,
  but this might be tricky considering the fact that the policy reasoner also needs to be able to access the directory
  in order to check the file permissions.

- Support multiple user mappings per location: Right now there is support in the code for multiple locations with each
  their own user mapping, as different sites will often comprise of different network shares. But as it is unclear right
  now how the volumes will be mounted on the reasoner container, it is hard to tell how such an implementation is best
  designed.

- Right now, this reasoner is both a module and a binary, but it should probably just be a binary. At this moment
  however, documenting binaries is tricky in rust. As soon as we find a better solution, this documentation should be
  moved to the binary itself. This might be useful in general, but particularly it is important to document reference
  implementations.

- LDAP / Active Directory support: Currently the Brane user to uid / gid mapping is embedded in the policy that is
  loaded at the moment the reasoner is started. The idea of mapping users to uid and gids is not unique though, these
  mappings can be sythesized from all sorts of resources. The most straightforward variant would be the loading of a
  `passwd(5)` file, but since we are aiming at distributed file systems this would probably be of limited use. In
  situations where file systems like NFS are often used, the users are store in Active Directory and accessed using
  LDAP. Writing such an adapter to function as the user map seems so be a valuable addition to this reasoner, as this
  would complete the picture of current existing systems and would allow a user of the reasoner to attach their already
  existing and managed file systems with the correct access control. This would significantly reduce the required
  investment of introducing policies in new Brane users.

- ACL support: Besides regular POSIX file permissions, many file systems also support [POSIX
  ACL](https://web.archive.org/web/20240210045229/https://www.usenix.org/legacy/publications/library/proceedings/usenix03/tech/freenix03/full_papers/gruenbacher/gruenbacher_html/main.html).
  This would be an obvious and very useful extension to this reasoner. Usage of these ACLs is by far less common that
  the regular POSIX permission it attempts to extend, but usage is also far from uncommon and potenial users of Brane,
  and more specifically the policy reasoner, could have used these ACLs on their existing file systems.

- The last point of future work is not specific to the POSIX reasoner, but more to the policy reasoner repository. There
  is a desperate need for more elaborate documentation. We attempted to create a nice start with this reasoner, but it
  can be quite daunting to figure out how every part of this system works. The author made a good effort to improve the
  documentation during the running of this project, and that helped a lot. That combined with the already existing
  implementation of the eFlint reasoner made this project possible. We hope that the POSIX (and [no_op](crate::no_op))
  reasoner can help guide future contributors in either extension of the current reasoners or the addition of new
  reasoner types.
