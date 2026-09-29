---
id: daemon
title: Daemon (yakd)
---

The first time that a yak command is run, yak starts a daemon process for
the current project. For subsequent commands, yak checks for the running
daemon process and, if found, uses the daemon to execute the command. Using the
yak daemon can save significant time as it enables yak to share cache between
yak invocations.

By default, there is 1 daemon per [project](./glossary.md#project) root, you can
run multiple daemons in the same project by specifying an
[isolation dir](./glossary.md#isolation-dir).

While it runs, the yak daemon process monitors the project's file system for
changes. The yak daemon excludes from monitoring any subtrees of the project
file system that are specified in the `[project].ignore` setting of
`.yakconfig`.

You can see detailed information about the status of the daemon by running
`yak status`.

## Killing or disabling the yak daemon

The yak daemon process is killed if `yak clean` or `yak kill` commands are
run. Note that they won't kill the daemon associated with custom isolation dirs.
To do that, run using the `--isolation-dir` option
(`yak --isolation-dir <dir> <command>`)

The daemon is also killed when:

- The `yak killall` command is run. By default it kills every yak process
  on the machine. Pass `--in-isolation-dir <dir>` to kill only the processes
  that use that isolation dir, and `--repo` to kill only the processes that run
  in the current repository.
- A command runs with a `yak` binary of a different version than the daemon.
  The client then restarts the daemon.
