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

The [`yak.file_watcher`](./yakconfig.md#file_watcher) setting selects the file
watcher. By default, the daemon uses Watchman when it is installed and the
`notify` watcher otherwise.

The operating system can drop file system events when many files change at
once, such as while a build writes its outputs. The `notify` watcher then
compares the size, modification time, and status change time of every file in
the project with its previous crawl, and invalidates only the files that
differ. The crawl skips `yak-out`, the `.git`, `.hg`, `.jj`, and `.sl`
directories, and the ignored subtrees. On macOS, the watcher also leaves
`yak-out` out of the events it receives.

The operating system delivers file system events after a delay, about 12
milliseconds for FSEvents on macOS. At the start of each command, the `notify`
watcher writes a file named `.yak-sync-<pid>-<number>` and waits for its
event before it reports changes, so the command sees every change made before it
started. The file goes in the `.git`, `.hg`, `.jj`, or `.sl` directory of the
project root when one exists, and in the project root otherwise, and the watcher
deletes it after its event arrives. If the event does not arrive within 10
seconds, or the file cannot be written, the watcher crawls the project as it
does after dropped events.

You can see detailed information about the status of the daemon by running
`yak status`.

## Killing or disabling the yak daemon

The yak daemon process is killed if `yak clean` or `yak kill` commands are
run. Note that they won't kill the daemon associated with custom isolation dirs.
To do that, run using the `--isolation-dir` option
(`yak --isolation-dir <dir> <command>`)

The daemon is also killed when:

- The `yak killall` command is run. By default it kills every yak process
  running in the current repository. Pass `--global` (`-g`) to kill the yak
  processes of every repository, and `--in-isolation-dir <dir>` to kill only
  the processes that use that isolation dir. It also kills the local actions
  that those processes started.
- A command runs with a `yak` binary of a different version than the daemon.
  The client then restarts the daemon.
