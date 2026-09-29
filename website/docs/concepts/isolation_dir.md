---
id: isolation_dir
title: Isolation Directory
---

# Isolation Directory

## What is an Isolation Directory?

An isolation directory is a core mechanism in Buck2 that enables multiple
independent daemon instances to run concurrently. Each Buck2 daemon operates
within its own isolation directory, creating a completely separated environment
for build processes.

The isolation directory serves as a fundamental boundary that:

- Separates cached artifacts between different daemon instances
- Provides independent build environments with no shared state
- Allows multiple Buck2 commands to run in parallel

## How Isolation Directories Work

### Physical Structure

The isolation directory exists as a subdirectory within the `yak-out` folder:

```
project_root/
└── yak-out/
    ├── v2/            # Default isolation directory
    │   ├── artifacts/
    │   ├── cache/
    │   └── ...
    ├── custom_name/   # Custom isolation directory
    │   ├── artifacts/
    │   ├── cache/
    │   └── ...
    └── ...
```

By default, Buck2 uses an isolation directory named `v2`, creating all build
outputs and metadata within `$PROJECT_ROOT/yak-out/v2`.

### Important Characteristics

1. **Independent Caching**:
   - Each isolation directory maintains its own separate cache
   - No cached artifacts or memory cache is shared between different isolation
     directories

2. **Command Execution Isolation**:
   - A single Buck2 daemon can generally execute only one command at a time
   - Different daemons with different isolation directories can execute commands
     concurrently

3. **Resource Implications**:
   - Using multiple isolation directories requires additional system resources
   - Each directory may duplicate build artifacts, consuming more disk space,
     memory, and potentially network bandwidth

:::warning **Resource Usage Warning**: Using multiple isolation directories can
significantly increase resource consumption due to duplicated caches and
artifacts. Each isolation directory requires its own memory, disk space, and
potentially network usage. :::

## When to Use Different Isolation Directories

Isolation directories are particularly useful in the following scenarios:

### 1. Developer Environment Tooling

Background services like Language Server Protocols (LSPs) can run in their own
isolation directory without interfering with manually triggered builds.

```sh
# Running LSP in its own isolation directory
$ yak --isolation-dir lsp lsp
```

### 2. Recursive Invocations

When Buck2 needs to be called from within another Buck2 process, using different
isolation directories prevents deadlocks and conflicts.

```sh
# Initial build
$ yak build //some:target

# Within this build, Buck2 might make another call using a different isolation dir
$ yak --isolation-dir recursive_dir //dependency:target
```

### 3. Parallel Workflows

When you need to run multiple independent build tasks simultaneously:

```sh
# Building the application in one terminal
$ yak build //app:binary

# Running tests in another terminal simultaneously
$ yak --isolation-dir test_dir test //app:tests
```

## How to Set the Isolation Directory

There are two ways to specify which isolation directory to use:

### 1. Command Line Argument

```sh
$ yak --isolation-dir DIRECTORY_NAME COMMAND [ARGS]
```

**Important**: The `--isolation-dir` argument must always appear immediately
after `yak`. For example, `yak build --isolation-dir v2 target` is not
valid.

### 2. Environment Variable

```sh
$ YAK_ISOLATION_DIR=DIRECTORY_NAME yak COMMAND [ARGS]
```

If not specified, the default isolation directory name is `v2`.

## Command Scope and Isolation Directories

Most Buck2 commands only operate within their specified isolation directory. For
example:

- `yak build` only builds using the specified isolation directory
- `yak clean` only cleans the specified isolation directory
- `yak kill` only kills the daemon associated with the specified isolation
  directory

There are exceptions, such as `yak killall`, which by default affects all
Buck2 processes regardless of their isolation directories. Passing
`--in-isolation-dir` to `yak killall` restricts it to processes using that
isolation directory, and `yak killall --repo` restricts it to processes
running in the current repository.

## Example Use Cases

### Typical Development Workflow

```sh
# Using the default isolation directory
$ yak build //app:binary
$ yak run //app:binary
```

### Running Background Analysis Services

```sh
# Start a language server in a dedicated isolation directory
$ yak --isolation-dir ide lsp &

# Continue with regular builds in the default isolation directory
$ yak build //app:binary
```

### Comparing Different Build Configurations

```sh
# Build with one set of configurations
$ yak --isolation-dir config1 build //app:binary

# Build with different configurations in a separate isolation directory
$ yak --isolation-dir config2 build //app:binary
```
