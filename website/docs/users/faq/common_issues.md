---
id: common_issues
title: Common Issues
---

## Why is stdin being swallowed?

Buck2 offers an interactive console by default.

To disable either use an env var: `BUCK_NO_INTERACTIVE_CONSOLE` or a flag:
`--no-interactive-console`

## Where is my output file?

To find the location of output for a target, use
`yak build //foo:bar --show-output`. This will print the output corresponding
to each built target, in this case `//foo:bar output_path`.

To only get the output path (without the target beforehand) you want to use
`yak build //foo:bar --show-simple-output`.

The resultant path is relative to the root of the repo (such as
`~/repo_root/...`). For the full path use `--show-full-output` or
`--show-full-simple-output`.

Note: in Buck1, the path is relative to the enclosing cell (such as
`~/repo_root/cell/...`).

## Why is Buck2 hanging?

If Buck2 seems to be doing nothing, it could be caused be a cycle in your
dependencies, which may cause Buck2 to hang (Buck2 does implement a form of
cycle detection, but it unfortunately has false negatives). You can confirm this
by running Buck1, which will report cycles properly.

## How do I get the commands Buck2 executed so I can reproduce them in isolation?

Run `yak log what-ran` after your build to see all the commands that were
executed.

## Are multiple concurrent commands supported?

Yes, they are supported. There are 2 types of concurrent commands: 1) parallel
invocations, and 2) recursive invocations.

**Parallel invocations:**

If the state of all the commands are the same, then they will run at the same
time. "State" is referring to the same configs and source files. If the state is
different amongst the commands, then yak will block the commands properly such
that the states do not interfere with each other. Different states are caused by
source file changes or config changes (ex: using a different mode).

**Recursive invocations:**

A recursive invocation is when an outer yak command ends up calling another
yak command as it's running. Recursive invocations are most commonly seen with
genrules and tests. For example:

- If you have a `genrule` where the command contains a `yak cquery`, and you
  build the genrule with `yak build`, you have a recursive invocation where
  the outer command is `yak build` and the inner command is `yak cquery`
- If you have a test which contains `yak build`, and you run your test with
  `yak test`, you have a recursive invocation where the outer command is
  `yak test` and the inner command is `yak build`

Recursive invocations should specify an
[`--isolation-dir`](../../concepts/isolation_dir.md), or else yak will return
an error.

## Why did my build OOM?

If your build OOMs, you can check the last actions running by using
`yak log whatup`. This will print the superconsole state at the moment the
event log ended, which will indicate what actions were being run (and consuming
memory) when your machine ran out of memory.

You can also use the `--after <millis>` option to see all open spans at a
certain point in time of the build.

## Why does my target not have any outputs?

If you see that your build succeeded, but the console message stated that your
target did not have any outputs, this means that the underlying rule did not
declare any outputs artifacts, defined as outputs declared in:

- `default_outputs` and/or `other_outputs` in `DefaultInfo`
- `cmd_args` in `RunInfo`
- `cmd_args` inside the `command` in `ExternalRunnerTestInfo`

For example, building a target which uses a `python_library` rule merely groups
source files together and does not generate any output artifacts such as a
python executable. You would need to build a `python_binary` which uses that
library in order to get an output.
