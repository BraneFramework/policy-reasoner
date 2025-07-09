[![Audit](https://github.com/BraneFramework/policy-reasoner/actions/workflows/audit.yml/badge.svg)](https://github.com/BraneFramework/policy-reasoner/actions/workflows/audit.yml)
[![CI](https://github.com/BraneFramework/policy-reasoner/actions/workflows/ci.yml/badge.svg)](https://github.com/BraneFramework/policy-reasoner/actions/workflows/ci.yml)
[![Coverage](https://codecov.io/github/BraneFramework/policy-reasoner/graph/badge.svg?token=HP09WOG59G)](https://codecov.io/github/BraneFramework/policy-reasoner)
<!-- [![Release](https://img.shields.io/github/release/braneframework/policy-reasoner.svg)](https://github.com/braneframework/policy-reasoner/releases/latest) -->
<!-- [![License: Apache-2.0](https://img.shields.io/github/license/braneframework/policy-reasoner.svg)](https://github.com/braneframework/policy-reasoner/blob/main/LICENSE) -->

# policy-reasoner
Implements the famous policy reasoner, known as `checker` in Brane terminology. Builds on top of reasoners like [eFLINT](https://gitlab.com/eflint) and meant to be queried by [Brane](https://github.com/braneframework/brane).

This project is complemented by the [Policy Reasoner GUI](https://github.com/braneframework/policy-reasoner-gui).


## Project structure
The project structure is described in [`ARCHITECTURE.md`](ARCHITECTURE.md).


## Installation
This section describes how to prepare your system to run the policy reasoner in isolation. If you want to use it in the context of Brane, note that it installs it by itself as part of the [`brane-chk`](https://github.com/BraneFramework/brane/tree/main/brane-chk)-service.

The various backends of the reasoner are executable as standalone binaries in the `examples/`-directory. They simply take a policy as input, run it, and show you the verdict that would be reported to services like Brane.

### eFLINT Haskell backend
The eFLINT Haskell backend relies on your system to have an `eflint-repl`-binary available to call to evaluate policy.

As such, install the Haskell implementation of eFLINT first. See the [README.md](https://gitlab.com/eflint/haskell-implementation) at the repository for up-to-date instructions. If you don't or cannot install Haskell to compile the binary, you can also use the Docker image; then, when running the example, be sure to specify a different command for running the `eflint-repl` binary (see the `--eflint-cmd`-option).

### eFLINT JSON backend
The eFLINT JSON backend relies on you running the `eflint-server`-binary from the <https://github.com/BraneFramework/eflint-server-go>-repository. The latest release will have precompiled binaries for Linux x86-64 and Arm 64-bit; or, alternatively, you can compile the binary yourself by running the steps below.

First, install the [Go compiler](https://go.dev/doc/install). Then, run the following commands to clone the appropriate repository and compile the server binary:
```sh
git clone https://github.com/BraneFramework/eflint-server-go && cd eflint-server-go
cd cmd/eflint-server
go build .
```
This will produce `cmd/eflint-server/eflint-server`, which is the binary that you should execute in the background of the policy reasoner binary. See how to do that below.

### no-op backend
The no-op backend does not require additional system dependencies. Simply compile it and you're ready to go.

### POSIX backend
The POSIX backend does not require additional system dependencies. Simply compile it and you're ready to go.


## Running
This section describes how to run the example binaries in the [`examples/`](examples/)-folder in this project.

We will once again enumerate the steps for every backend binary.

### eFLINT Haskell backend
To run the eFLINT Haskell backend, run:
```sh
cargo run --example eflint-haskell --features eflint-haskell-reasoner,file-logger -- <EFLINT FILE TO RUN>
```
Optionally, if your downloaded `eflint-repl` binary is executable by another command than simply that, give it with the `--eflint-cmd`-option. For example, when running the Docker version:
```sh
cargo run --example eflint-haskell --features eflint-haskell-reasoner,file-logger -- <EFLINT FILE TO RUN> --eflint-cmd 'docker run -it --rm eflint repl'
```

The reasoner will evaluate the policy file specified, and analyse the trace of results produced by the binary to give a verdict. It will be shown to you on the terminal.

### eFLINT JSON backend
To run the eFLINT JSON backend, first run the `eflint-server` binary in a separate terminal window:
```sh
./eflint-server
```
It will then report to you the address where the server may be reached. Note it down, and then run the example in another terminal window:
```sh
cargo run --example eflint-json --features eflint-json-reasoner,eflint-to-json,file-logger -- <EFLINT FILE TO RUN> --address <ADDRESS>
```
where `<ADDRESS>` is the address you copied from the `eflint-server` output.

The eFLINT file can either be given in eFLINT DSL syntax, or the JSON syntax. You can use the `--dsl` and `--json` flags to tell the binary which of the two you are giving. Omitting it will default to `--dsl`.

If you are specifying the DSL syntax, the binary will attempt to automatically download the `eflint-to-json` binary for your system from the internet. It it fails, or you have another reason for preferring otherwise, use the `--eflint-path`-option to refer to an existing binary instead.

The reasoner will evaluate the policy file specified, and analyse the trace of results produced by the binary to give a verdict. It will be shown to you on the terminal.

### no-op backend
The no-op backend is the simplest to use, as it does not do anything. Simply hit:
```sh
cargo run --example no-op --features no-op-reasoner,file-logger
```
and let it do the rest. It will tell you the verdict of the reasoner at the end of the file (which is also OK).

### POSIX backend
The POSIX backend does not accept a single policy file, but instead a workflow that represents the scenario and a config that represents the policy.

The workflow file should be given as a JSON serialization of the `Workflow`-struct provided by the [`workflow`](lib/workflow/)-crate. The best documentation is the Rust struct itself; or, alternatively, refer to examples in the [`tests/workflow`](tests/workflow/)-folder.

The config file is also given as JSON, but has a simpler syntax. For example (using illegal comments to show you what's what):
```json
{
    // The location identifier of the worker for which this policy is written.
    "id": "amy",
    // Maps datasets to policy information
    "data": {
        // The workflow identifier of the target dataset
        "some_dataset": {
            // The path where it resides. This may actually be some other file
            // than the dataset itself if you want to separate the permissions.
            "path": "/home/pi/some_dataset",
            // Defines how _workflow users_ map to _POSIX users_
            "user_map": {
                "bob": {
                    // This is the user ID of the user to which `bob` maps
                    "uid": 1000,
                    // This is a list of group IDs which the user is
                    // additionally a member of.
                    "gids": [1000, 1001, 1002]
                }
            }
        }

        // More data mappings ...
    }
}
```

Once you're created a workflow and a config file, you can run the reasoner on them with:
```sh
cargo run --example posix --features posix-reasoner,file-logger,serde,workflow -- --workflow <WORKFLOW_FILE_PATH> --config <CONFIG_FILE_PATH>
```

If you omit the `--workflow` option, the binary will attempt to read it from stdin instead.


## Contribution
Contributions to this project are welcome! If you have thoughts, suggestions or encounter bugs, you can leave an issue on this repository's [issue-page](https://github.com/braneframework/policy-reasoner/issues). If you have concrete fixes already implemented, you can also create [pull requests](https://github.com/braneframework/policy-reasoner/pulls) directly.

An overview the structure of this repository can be found in [Architecture.md](./ARCHITECTURE.md)

## License
This project is licensed under the Apache 2.0 license. See [LICENSE](./LICENSE) for more details.
