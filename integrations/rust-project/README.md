# rust-project

The `rust-project` tool is intended to read dependency information from `yak`,
and generate a
[rust-project.json](https://rust-analyzer.github.io/manual.html#non-cargo-based-projects)
file for use with `rust-analyzer`.

A project that builds its Rust code with `yak` has no Cargo manifests for
`rust-analyzer` to read, so `rust-project` describes the project structure
instead.

# Usage

To generate a `rust-project.json` file using `rust-project`, supply it with one
or more `yak` targets. The following command, run from the root of this
repository, creates a `rust-project.json` for the `rust-project` target itself:

```bash
rust-project develop //integrations/rust-project:rust-project
```

The `develop` command will write to the current working directory.

Placing `rust-project.json` at the root of the Rust project directory will allow
`rust-analyzer`-the-LSP-engine to find and use it for analysis.

To emit logs, set the environment variable `RUST_LOG` to a value. Supported
syntax is described
[here](https://docs.rs/tracing-subscriber/latest/tracing_subscriber/struct.EnvFilter.html).
