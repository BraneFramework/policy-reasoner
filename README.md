# policy-reasoner
Implements the famous policy reasoner, known as `checker` in Brane terminology. Builds on top of reasoners like [eFLINT](https://gitlab.com/eflint) and meant to be queried by [Brane](https://github.com/braneframework/brane).

This project is complemented by the [Policy Reasoner GUI](https://github.com/braneframework/policy-reasoner-gui).


## Installation
This section describes how to install the policy reasoner in isolation. It is automatically built as the `brane-chk` service in [Brane](https://github.com/braneframework/brane).


### Compiling `eflint-server` (and `eflint-json`)
By default, this repository uses a connector that runs the eFLINT JSON Specification. That means that in addition to the policy reasoner itself, the _actual_ eFLINT reasoner also has to be built.

This project relies on the [eFLINT GO server](https://github.com/Olaf-Erkemeij/eflint-server) (or rather, the fork [here](https://github.com/epi-project/eflint-server-go)).

To install, clone the repository and use the [Go compiler](https://go.dev/doc/install) to build it:
```bash
git clone https://github.com/epi-project/eflint-server-go && cd eflint-server-go
cd cmd/eflint-server
go build .
```
This will produce `cmd/eflint-server/eflint-server`, which is the binary that must be run later.

If you are on Linux x86/64, you can also simply download the precompiled binary for the `eflint-server` [here](https://github.com/epi-project/eflint-server-go/raw/main/eflint-server).


### Setting up the database
The database setup is done automatically by `build.rs` if it isn't found already.

If you somehow have to setup a manual database, you should first install the `diesel-cli` that can apply the migrations though the command-line:
```bash
cargo install diesel_cli --no-default-features --feature sqlite
```

Next, run the following commands to create the `data/policy.db`-file:
```bash
mkdir data && touch data/policy.db
diesel migration run --database-url data/policy.db
```


## Running
Running is quite straightforwardly done using Cargo's `run`-subcommand. However, there are some details, so read below:

### Generating keys & tokens
The policy reasoner features two endpoints, the _deliberation endpoint_, and the _policy expert endpoint_ or _management endpoint_ (see [below](#usage)). Both of these are protected by the [JSON Web Key Sets](https://auth0.com/docs/secure/tokens/json-web-tokens/json-web-key-sets) [`examples/config/jwk_set_delib.json`](examples/config/jwk_set_delib.json) and [`examples/config/jwk_set_expert.json`](examples/config/jwk_set_expert.json), respectively.

It is recommended to change the default provided keys, as these are for testing purposes only. You can do so using the `key-manager` tool:
```rust
cargo run --package key-manager -- generate jwk ./examples/config/jwk_set_delib.json
cargo run --package key-manager -- generate jwk ./examples/config/jwk_set_expert.json
```

Then, you can generate tokens that can be used to login using:
```bash
cargo run --package key-manager -- generate jwt -k ./examples/config/jwk_set_delib.json ./jwt_delib.json <USER> <SYSTEM> <DURATION>
cargo run --package key-manager -- generate jwt -k ./examples/config/jwk_set_expert.json ./jwt_expert.json <USER> <SYSTEM> <DURATION>
```
where `<USER>` should be the name of the user using the JWT, `<SYSTEM>` the name of the system they are accessing through (if it matters; otherwise, just use the same value as `<USER>`) and `<DURATION>` is the time the JWT is valid for. For example:
```bash
cargo run --package key-manager -- generate jwt -k ./examples/config/jwk_set_delib.json ./jwt_delib.json amy site-a-worker 31d
cargo run --package key-manager -- generate jwt -k ./examples/config/jwk_set_expert.json ./jwt_expert.json amy site-a-worker 31d
```


### Backend reasoner
If you're running a backend reasoner like eFLINT's, run that first. Simply run:
```bash
./cmd/eflint-server/eflint-server
```
from the root of the closed `eflint-server-go`-repository (see [above](#compiling-eflint-server-and-eflint-json)). Note that you have to leave this process running for as long as the policy reasoner itself runs.


### The Policy Reasoner
To run the `policy-reasoner`, use the `cargo run`-command to build and execute it automatically.

Note that doing so will:
- Automatically compile the eFLINT DSL interface to eFLINT JSON; and
- Generate the `data/policy.db` file if it doesn't already exist.

If the default settings work for you, run:
```bash
cargo run --release
```
to launch the reasoner.

By default, the former downloads the precpompiled Linux x86/64 binary from the [fork](https://github.com/epi-project/eflint-server-go). If you're in another system, compile the `eflint-to-json` binary yourself (same procedure as for the `eflint-server`, but replace the latter term with the former in the process) and then give it as an environment variable:
```bash
EFLINT_TO_JSON_PATH="<path/to/eflint-server-go>/cmd/eflint-to-json/eflint-to-json" cargo run --release
```


## Usage
> The [Policy Reasoner GUI](https://github.com/braneframework/policy-reasoner-gui) provides an alternative interface to the Policy Reasoner. You can consult that repository for more information on using it.

The Policy Reasoner implements two endpoints:
- The _deliberation endpoint_ is used by systems using the reasoners to consult policy; and
- The _policy expert endpoint_ or _management endpoint_ is used by policy experts to manage policy.

Both require different a JWT to access, which you can generate by following the [section above](#generating-keys--tokens). These should be given as [Bearer-tokens](https://swagger.io/docs/specification/authentication/bearer-authentication/) in the Authorization header of every request, e.g.,
```http
Authorization: Bearer <token>
```

With the keys set, you can access the following endpoints:
- Deliberation API
  - `POST v1/deliberation/execute-workflow`: Ask if the reasoner would be OK with participating in the given workflow.  
    - As a body, a JSON object should be given with:
      - `use_case`: A string that defines the use-case for which this request is done for. Currently only relevant when using the `BraneApiStateResolver` to choose which central registry to ask for state.
      - `workflow`: A nested JSON Object that represents Brane's [WIR](https://wiki.enablingpersonalizedinterventions.nl/specification/spec/wir/introduction.html) (i.e., the input workflow).
    - The response is a JSON object with:
      - `verdict`: The verdict of the checker, which is either a JSON string `"allow"` or a JSON string `"deny"`.
      - `verdict_reference`: A JSON String with a UUID that can be traced back in the logs to explain the verdict.
      - `signature`: A JSON string that carries the checker's signature (unimplemented, currently dummy implementation).
  - `POST v1/deliberation/execute-task`: Ask if the reasoner would be OK with executing a particular task in the given workflow.  
    - As a body, a JSON object should be given with:
      - `use_case`: A string that defines the use-case for which this request is done for. Currently only relevant when using the `BraneApiStateResolver` to choose which central registry to ask for state.
      - `workflow`: A nested JSON Object that represents Brane's [WIR](https://wiki.enablingpersonalizedinterventions.nl/specification/spec/wir/introduction.html) (i.e., the input workflow).
      - `task_id`: The identifier of the task that is asked about. Given as an array of two elements, with either `<main>` or the function ID of a specific function as first element, and the edge index within that function as second element (see the [WIR](https://wiki.enablingpersonalizedinterventions.nl/specification/spec/wir/introduction.html) for more information).
    - The response is a JSON object with:
      - `verdict`: The verdict of the checker, which is either a JSON string `"allow"` or a JSON string `"deny"`.
      - `verdict_reference`: A JSON String with a UUID that can be traced back in the logs to explain the verdict.
      - `signature`: A JSON string that carries the checker's signature (unimplemented, currently dummy implementation).
  - `POST v1/deliberation/access-data`: Ask if the reasoner would be OK with transferring a particular dataset to be used as input to the given task in the given workflow OR as result of the given workflow.  
    - As a body, a JSON object should be given with:
      - `use_case`: A string that defines the use-case for which this request is done for. Currently only relevant when using the `BraneApiStateResolver` to choose which central registry to ask for state.
      - `workflow`: A nested JSON Object that represents Brane's [WIR](https://wiki.enablingpersonalizedinterventions.nl/specification/spec/wir/introduction.html) (i.e., the input workflow).
      - `task_id`: An _optional_ identifier of the task that is asked about. Given as an array of two elements, with either `<main>` or the function ID of a specific function as first element, and the edge index within that function as second element (see the [WIR](https://wiki.enablingpersonalizedinterventions.nl/specification/spec/wir/introduction.html) for more information).  
        If this identifier is omitted, it means that instead this workflow returns a result to the user submitting it and we're asking if that transfer would be OK.
      - `data_id`: The ID of the dataset/intermediate result that we're asking about.
    - The response is a JSON object with:
      - `verdict`: The verdict of the checker, which is either a JSON string `"allow"` or a JSON string `"deny"`.
      - `verdict_reference`: A JSON String with a UUID that can be traced back in the logs to explain the verdict.
      - `signature`: A JSON string that carries the checker's signature (unimplemented, currently dummy implementation).
- Management API
  - `GET v1/management/policies`: Retrieve the list of all policy versions on the reasoner.
    - No body is required for this request.
    - The call returns a JSON Array of policy versions stored on the reasoner, each of which is a JSON Object with:
      - `creator`: An _optional_ JSON String with the name of the user that submitted the workflow.
      - `created_at`: The time the policy was uploaded.
      - `version`: The ID of this version, as a formatted time string.
      - `version_description`: The description for this specific version.
      - `reasoner_connector_context`: The hash of the context for which this policy is valid.
  - `POST v1/management/policies`: Push a new policy version to the reasoner.
    - The body of this request should be a JSON Object with:
      - `description`: An _optional_ JSON String that provides a generic description for policy in this reasoner. You can expect this one to be duplicate across versions.
      - `version_description`: A JSON String that provides a short description or commit message for this version of the policy.
      - `content`: A JSON Array with nested JSON Objects with:
        - `reasoner`: The string identifier that determines the reasoning backend for which this policy is meant (for the eFLINT backend, this is `eflint-json`).
        - `reasoner_version`: A JSON String that denotes the version of the backend reasoner for which this policy is meant (for the eFLINT backend, this is `0.1.0`).
        - `content`: The content of the policy. This is arbitrary other JSON, and will be passed as-is to the backend connector that translates it to the reasoner implemented.
    - The request returns a JSON Object with the same fields to confirm the policy has been uploaded.
  - `GET v1/management/policies/:id`: Retrieve the contents of a particular policy version with identifier `:id`.
    - No body is required for this request.
    - A JSON Object is returned that contains the requested policy. The fields are indentical as returned by `POST v1/management/policies`.
  - `GET v1/management/policies/active`: Get the ID of the currently active policy.
    - No body is required for this request.
    - A JSON Object is returned that contains the requested policy. The fields are indentical as returned by `POST v1/management/policies`.
  - `PUT v1/management/policies/active`: Update the currently active policy.  
    - The body of this request should be a JSON Object with:
      - `version`: A JSON integer that is the ID of the policy to set active.
    - A JSON Object is returned that contains the policy to which the reasoner has switched. The fields are indentical as returned by `POST v1/management/policies`.
  - `DELETE v1/management/policies/active`: De-active the currently active policy, reverting to "deny all" policy.  
    - No body is required for this request.
    - No result is returned by this request.

For example, using [curl](https://curl.se/):
```bash
# Run this first to set the keys!
export JWT_DELIB="$(cat ./jwt_delib.json)"
export JWT_EXPERT="$(cat ./jwt_expert.json)"
```

```bash
# Check a workflow!
curl -X POST -H "Authorization: Bearer $JWT_DELIB" -H "Content-Type: application/json" -d "@tests/deliberation/execute-workflow.json" localhost:3030/v1/deliberation/execute-workflow
```

```bash
# Push a new policy!
curl -X POST -H "Authorization: Bearer $JWT_EXPERT" -H "Content-Type: application/json" -d "@tests/management/add-tautology.json" localhost:3030/v1/management/policies
```

```bash
# Make a policy active!
curl -X PUT -H "Authorization: Bearer $JWT_EXPERT" -H "Content-Type: application/json" -d '{ "version": 1 }' localhost:3030/v1/management/policies/active
```


## Contribution
Contributions to this project are welcome! If you have thoughts, suggestions or encounter bugs, you can leave an issue on this repository's [issue-page](https://github.com/braneframework/policy-reasoner/issues). If you have concrete fixes already implemented, you can also create [pull requests](https://github.com/braneframework/policy-reasoner/pulls) directly.

An overview the structure of this repository can be found in [Architecture.md](./ARCHITECTURE.md)

## License
This project is licensed under the Apache 2.0 license. See [LICENSE](./LICENSE) for more details.
