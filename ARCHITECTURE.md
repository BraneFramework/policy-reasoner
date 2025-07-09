Policy reasoner architecture
============================

This project is organised as a collection of crates that implement various aspects of various instantiations of a `policy-reasoner`. They are separate crates instead of feature-gated modules in order to tame the wildy varying dependencies a little.

The crates doing the actual implementations can be found in the [`lib/`](lib/)-folder. Then, the [`src/`](src/)-folder provides a collection library that dynamically includes the crates based on features. Finally, [`examples/`](examples/) contains various example instantiations of the reasoner for different backends. They aren't (yet) exhaustive in terms of _all_ possible instantiations, but they do serve as an introduction to how to use and install each backend.

## Libraries
The following libraries are provided by the `policy-reasoner` workspace, grouped by function:
- [_reasoners_](lib/reasoners/) implement the `ReasonerConnector`-trait, which abstracts over specific backends.
  - [`eflint-haskell`](lib/reasoners/eflint-haskell) contributes a wrapper around the [eFLINT Haskell implementation](https://gitlab.com/eflint/haskell-implementation). It accepts eFLINT's DSL.
  - [`eflint-json`](lib/reasoners/eflint-json) contributes a wrapper around Olaf's [eFLINT GO-implementation](https://github.com/Olaf-Erkemeij/eflint-server/) (or, more precisely, [our fork](https://github.com/BraneFramework/eflint-server-go)). It accepts [eFLINT's JSON specification](https://gitlab.com/eflint/json-specification).
  - [`no-op`](lib/reasoners/no-op) contributes a dummy reasoner that blindly accepts anything. It is mostly used for debugging purposes.
  - [`posix`](lib/reasoners/posix) contributes a wrapper around Unix's filesystem to use its permissions for deciding dataset access. It is based on the work done by [Daniel Voogsgerd, ]().
- [_resolvers_](lib/resolvers/) implement the `StateResolver`-trait, which is responsible for providing current information about the runtime system to a static policy.
  - [`file`](lib/resolvers/file) contributes a resolver that reads the current system state from a file. This one is mostly for debugging, though; usually, systems implement their own resolvers to discover the current state.
- [_loggers_](lib/loggers/) implement the `AuditLogger`-trait, which is triggered with auditable information of specific events in the reasoner and can store it in implementation-specific ways.
  - [`file`](lib/loggers/file) contributes an implementation that writes the events to a file.
  - [`no-op`](lib/loggers/no-op) contributes a dummy implementation that doesn't write any audit event. This is used for debugging, or in scenarios where no audit trail is desired.

Then there are also a few miscellaneous, auxillary libraries:
- [`eflint-to-json`](lib/eflint-to-json) contributes a wrapper around Olaf's [eFLINT to eFLINT JSON compiler](https://github.com/Olaf-Erkemeij/eflint-server/) (or more precisely, [our fork](https://github.com/BraneFramework/eflint-server-go)). This allows frontends to also use the eFLINT DSL with the [`eflint-json`](lib/reasoners/eflint-json) backend.
- [`spec`](lib/spec) contributes the core traits of the `policy-reasoner` mentioned above.
- [`workflow`](lib/workflow) contributes a specification of an abstract workflow that can be used to inform the policy to the task at hand.
