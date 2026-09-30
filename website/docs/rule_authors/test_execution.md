---
id: test_execution
title: Test Execution
---

Test execution in yak is a collaboration with a separate test runner process.

yak ships with a built-in test runner, which runs as
`yak internal-test-runner`. It receives the commands defined by
`ExternalRunnerTestInfo` and runs them. Exit code zero means the test passed,
and any other exit code means it failed.

Setting `[test] v2_test_executor` to the path of an executable makes yak use
that executable as the test runner. The built-in runner in
[`app/yak_test_runner`](https://github.com/rdeusser/yak/tree/main/app/yak_test_runner)
is a sample to start from. A more capable test runner can take on these
responsibilities:

- **Translation**:
  - Understands the output formats of various supported test frameworks. This is
    used to identify test cases and collect test results.
  - Understands, to an extent, the input formats. For example, given a test
    case, the runner can identify what command needs to run to execute just
    that test.
- **Orchestration**:
  - Discovers what tests should run, under a number of configurations.
  - Separates listing of tests (identifying what tests exists in a test target)
    and execution (running specific tests within that target).
  - Coordinates the execution of tests. For example, it may request retries, or
    choose to bundle multiple tests in a single execution (or not).
  - Reports test results.

In yak, rules interact with the test runner via a provider called
`ExternalRunnerTestInfo`.

## Anatomy of a test run

When a user runs `yak test $targets`:

- yak identifies all matching targets that have an `ExternalRunnerTestInfo`.
- yak builds all the artifacts referenced by those targets (this will likely
  change eventually to build them only if they are used).
- yak then notifies the test runner that those tests exist. Currently, the
  test runner receives a subset of `ExternalRunnerTestInfo`.
- The test runner can request command execution from yak to list and execute
  tests.
- When it receives command results from yak, the test runner may fire off
  events that the end-user will see (such as test results), upload logs
  externally, request further executions, and so on.

:::note

If more than one target is being built, test building and execution will proceed
concurrently.

:::

## Information available on `ExternalRunnerTestInfo`

As noted, rules communicate their testing capabilities via
`ExternalRunnerTestInfo`. There are a number of fields available on
`ExternalRunnerTestInfo` to control how a given target is tested, as detailed in
the following sub-sections.

### Fields exposed to the test runner

The following list shows what is available in `ExternalRunnerTestInfo`, with
which the test runner can interact:

- `type` - a string key that defines the type of test this is.
- `command` and `env` - respectively, a list and a key-value mapping of
  arguments. They are not always visible to the test runner (for more
  details, see
  [Verbatim arguments and handles](#verbatim-arguments-and-handles), below).
- `labels` - a set of string labels to pass to the test runner.
- `contacts` - a list of contacts for the tests, usually the owning team.
- `executor_overrides` - a key-value mapping of executor configurations that the
  test runner can use when requesting execution from yak.
- `local_resources` - a key-value mapping from resource type to optional
  `LocalResourceInfo` provider. Provider is used for initialization of that
  resource type. If the value is `None` resource type is ignored even though
  test runner required it. For context see
  [Local Resources For Tests Execution](local_resources.md).

### Fields pertinent for Remote Execution

For compatibility with Remote Execution (RE), there are two fields that rules
should set in their `ExternalRunnerTestInfo` if they should be run on RE:

- `use_project_relative_paths` - if `true` (the default is `true`), yak will
  produce relative paths. If not, it'll produce absolute paths.
- `run_from_project_root` - if `true` (the default is `true`), tests will run
  from the project root (their `cwd` will be the project root, which is
  the same as all build commands). If `false`, it'll be the cell root.

`working_directory` names a directory for the test to run in, such as
`cmd_args(srcs_dir, "/pkg", delimiter = "")` for a directory inside an output of
the rule. It takes precedence over `run_from_project_root`, and yak builds the
artifacts it names before the test runs. Paths relative to the project root do
not resolve from another directory, so a test with a `working_directory` should
also set `use_project_relative_paths = False`. `rust_test` sets it when
`run_from_manifest_dir` is set.

Note that passing `--unstable-allow-all-tests-on-re` to `yak test` will
override those fields and set them to `true`, since they are a pre-requisite to
run on RE. In contrast, passing `--unstable-allow-compatible-tests-on-re` will
only allow tests that already set both those fields to `true` to execute on RE.

Also note that when `executor_overrides` are set, if an executor override is
used and results in execution on RE, it'll happen on RE unconditionally.
Therefore, it's a good idea to set those fields if RE-only executor overrides
are provided.

## Verbatim arguments and handles

As noted above, the test runner only interacts with a subset of arguments
provided by rules in `ExternalRunnerTestInfo`. The reason for this is that the
test runner doesn't get to access, for example, artifacts, that yak knows
about.

Consider the following example:

```python
binary = ctx.attrs.dep[RunInfo]
test_info = ExternalRunnerTestInfo(command = [binary, "run-tests"], ...)
```

When yak actually runs this command, `binary` is expanded to a path (and
possibly to more args). yak would also account for any hidden arguments and
make those available where the command is executed. It is important for yak to
retain this capability when running with the test runner.

To that end, all non-trivial arguments present in `command` (and in the values
of `env`), such as `cmd_args` or `RunInfo`, are exposed to the test runner as
opaque handles, and simple string arguments are passed as-is to the test runner.

This means that the test runner would see the command described above as:

```python
[ArgHandle(index = 0), Verbatim("foobar")]
```

When requesting execution from yak, the test runner can use the `ArgHandle`
and yak will swap it back for the underlying value that was set on the
provider.

This allows the test runner to introspect and modify parts of the command lines
it receives, as long as it doesn't need to access the actual text value of
non-verbatim arguments. Usually, this works out to be sufficient (or can be made
sufficient with a bit of refactoring in the test runner).

## Execution Configurations

By default, tests execute using the execution configuration of the associated
target. This is the execution configuration that would be used for run actions
(`ctx.actions.run`) declared in the same target. This is a default that actually
makes little sense but works out as long as cross-compiling is not the norm.

The default breaks down when the steps of a test need different platforms. For
example, building an iOS test needs Xcode, listing its test cases can run on any
platform, and running them needs a simulator.

To support this, `ExternalRunnerTestInfo` allows specifying override platforms,
which are given a name. The test runner can request execution on them by passing
their name when it sends execution requests to yak, as shown in the following
code:

```python
ExternalRunnerTestInfo(
  executor_overrides = {
      "ios-simulator": CommandExecutorConfig(
          local_enabled = False,
          remote_enabled = True,
          remote_execution_properties = {
              "platform": "ios-simulator",
              "subplatform": "iPhone 8.iOS 15.0",
              "xcode-version": "xcodestable",
          },
      ),
      "static-listing": CommandExecutorConfig(local_enabled = True, remote_enabled = False),
  },
  ...
)
```

The default execution platform can also be overridden:

```python
ExternalRunnerTestInfo(
  default_executor = CommandExecutorConfig(
    local_enabled = False,
    remote_enabled = True,
    remote_execution_properties = {
        "platform": "ios-simulator",
        "subplatform": "iPhone 8.iOS 15.0",
        "xcode-version": "xcodestable",
    },
  ),
  ...
)
```

## Working Directory

Tests can be run from the cell root by setting `run_from_project_root = False`.

To produce paths relative to the cell root for use by tests, use
`relative_to(ctx.label.cell_root)` on `cmd_args`.

## Caching Test Listings

A test runner can start with a **listing** step, which runs the test binary to
discover the test cases it contains. The runner marks each listing request as
cacheable or not, and yak caches the result of a cacheable listing at two
levels:

1. The daemon keeps the result in its DICE computation graph. If you run
   `yak test` on the same target again without changes, yak skips the
   listing, so it does not appear in `yak log what-ran` output.
2. yak looks the listing command up in the remote action cache before it runs
   the command, and uploads the result after running it, so the result survives
   a daemon restart. The `CommandExecutorConfig` that runs the listing controls
   both steps. Its `remote_cache_enabled` field enables the lookup, and its
   `allow_cache_uploads` field enables the upload of a listing that ran locally.

yak caches only listings that exit with code 0, and runs a failed listing
again on the next request.

The built-in test runner has no listing step, so this caching applies only to
test runners set with `[test] v2_test_executor`.
