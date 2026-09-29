#!/usr/bin/env python3
"""Hand edits of the second part of milestone 6 that follow rename_code_prose.py.

Run from the repository root after `rename_code_prose.py --apply`:

    python3 path/to/manual_edits_code.py

Each edit names its file, the exact text it replaces, and how many times that
text occurs, so a rerun on a changed tree fails instead of editing blindly.
"""

import subprocess
import sys

failures = []


def edit(path, old, new, count=1):
    with open(path, encoding="utf-8") as f:
        text = f.read()
    n = text.count(old)
    if n != count:
        failures.append(f"{path}: expected {count} of {old!r}, found {n}")
        return
    with open(path, "w", encoding="utf-8") as f:
        f.write(text.replace(old, new))


def git(*args):
    subprocess.run(["git", "-c", "submodule.recurse=false", *args], check=True)


# The binary names itself in HTTP requests and in the project that `yak init`
# writes.
edit(
    "app/buck2_http/src/client.rs",
    'const DEFAULT_USER_AGENT: &str = "Buck2";',
    'const DEFAULT_USER_AGENT: &str = "yak";',
)
edit(
    "app/buck2_client/src/commands/init.rs",
    'cmd = \\"echo BUILT BY BUCK2> $OUT\\",',
    'cmd = \\"echo BUILT BY YAK> $OUT\\",',
    count=2,
)
edit(
    "games/src/games/menu.rs",
    '//! - **Animated title**: "BUCK2 GAMES" with a cycling dot animation',
    '//! - **Animated title**: "YAK GAMES" with a cycling dot animation',
)
edit(
    "games/src/games/menu.rs",
    'let title = format!(" BUCK2 GAMES{:<17}", dots);',
    'let title = format!(" YAK GAMES{:<19}", dots);',
)
# The banner keeps its width.
edit(
    "app/buck2_client_ctx/src/subscribers/superconsole/debugger.rs",
    '"     ******** yak Starlark Debugger is attached **********"',
    '"     ********* yak Starlark Debugger is attached ***********"',
)

# rust-analyzer runs the tests of a crate with the `yak` binary.
edit(
    "integrations/rust-project/src/buck.rs",
    'program: "buck".to_owned(),',
    'program: "yak".to_owned(),',
)
edit(
    "integrations/rust-project/src/buck.rs",
    "    /// Invoke `buck` with the given subcommands.\n",
    "    /// Invoke `yak` with the given subcommands.\n",
)

# Milestone 1 removed the Buck UI URL, and the client prints only the build ID.
for path in (
    "app/buck2_client/src/commands/install.rs",
    "app/buck2_client/src/commands/test.rs",
):
    edit(path, "use crate::commands::build::print_buck_ui;", "use crate::commands::build::print_build_id;")
# The import moves to its sorted place.
edit(
    "app/buck2_client/src/commands/run.rs",
    "use crate::commands::build::print_buck_ui;\nuse crate::commands::build::print_build_failed;\n",
    "use crate::commands::build::print_build_failed;\nuse crate::commands::build::print_build_id;\n",
)
for path in (
    "app/buck2_client/src/commands/install.rs",
    "app/buck2_client/src/commands/run.rs",
    "app/buck2_client/src/commands/test.rs",
):
    edit(
        path,
        "print_buck_ui(&console, ctx, events_ctx.used_superconsole)?;",
        "print_build_id(&console, ctx, events_ctx.used_superconsole)?;",
    )
edit(
    "app/buck2_client/src/commands/build.rs",
    "print_buck_ui(&console, ctx, events_ctx.used_superconsole)?;",
    "print_build_id(&console, ctx, events_ctx.used_superconsole)?;",
)
edit(
    "app/buck2_client/src/commands/build.rs",
    """/// Re-prints the Buck UI URL at command end, but only when a superconsole was
/// actually constructed for the command (`used_superconsole`). Superconsole's
/// live area showed the URL during the command but clears on exit, so without
/// the re-print the URL would be gone from scrollback.""",
    """/// Re-prints the build ID at command end, but only when a superconsole was
/// actually constructed for the command (`used_superconsole`). Superconsole's
/// live area showed the ID during the command but clears on exit, so without
/// the re-print the ID would be gone from scrollback.""",
)
edit(
    "app/buck2_client/src/commands/build.rs",
    "pub(crate) fn print_buck_ui(",
    "pub(crate) fn print_build_id(",
)
edit(
    "app/buck2_client_ctx/src/common/ui.rs",
    "    /// Used to decide whether to print Buck UI / Build ID at the end of a build,\n",
    "    /// Used to decide whether to print the build ID at the end of a build,\n",
)
edit(
    "app/buck2_client_ctx/src/subscribers/superconsole.rs",
    "        // Buck UI/Build ID + Test UI, each on one line\n",
    "        // Build ID + Test UI, each on one line\n",
)
edit(
    "app/buck2_client_ctx/src/subscribers/superconsole.rs",
    "        // Buck UI header, Buck UI value, Test UI header, Test UI value\n",
    "        // Build ID header, Build ID value, Test UI header, Test UI value\n",
)
edit(
    "integrations/rust-project/src/buck.rs",
    'if let Some(pos) = line.find("Buck UI") {',
    'if let Some(pos) = line.find("Build ID") {',
    count=2,
)

# BXL expands to the extension language of yak.
edit(
    "app/buck2_build_api/src/bxl.rs",
    "//! bxl is the Buck Extension Language, allowing any integrator to write Starlark code that\n",
    "//! bxl is the extension language of yak, allowing any integrator to write Starlark code that\n",
)

# "Buck 2" with a space names the tool.
edit(
    "app/buck2_core/src/configuration/data.rs",
    "/// We don't use derive(Hash) here because we build Buck 2 on two different versions of Rustc at\n",
    "/// We don't use derive(Hash) here because we build yak on two different versions of Rustc at\n",
)
edit(
    "app/buck2_daemon/src/schedule_termination.rs",
    "/// Our tests sometimes don't exit Buck 2 cleanly, and they might not get an oppportunity to do so\n",
    "/// Our tests sometimes don't exit yak cleanly, and they might not get an oppportunity to do so\n",
)
edit(
    "app/buck2_execute/src/directory.rs",
    "        // the deps! In practice, this tends to not happen in Buck 2 because we always dereference\n",
    "        // the deps! In practice, this tends to not happen in yak because we always dereference\n",
)
edit(
    "app/buck2_external_cells/src/bundled.rs",
    "Consider `export NANO_PRELUDE=<buck2 repository>/tests/e2e_util/nano_prelude`",
    "Consider `export NANO_PRELUDE=<yak repository>/tests/e2e_util/nano_prelude`",
)

# Messages and comments that the rules turned into something else.
edit(
    "app/buck2_interpreter_for_build/src/interpreter/interpreter_for_dir.rs",
    '#[error("Tabs are not allowed in yak files: `{0}`")]',
    '#[error("Tabs are not allowed in build files: `{0}`")]',
)
edit(
    "app/buck2_error/src/classify.rs",
    "// yak is the fallback/default source area, use the first non-buck2 source area.\n",
    "// `ErrorSourceArea::Buck2` is the fallback/default source area, use the first other source area.\n",
)
edit(
    "app/buck2_re_configuration/src/lib.rs",
    "/// The remote execution configuration, read from the `buck2_re_client` yakconfig section.\n",
    "/// The remote execution configuration, read from the `yak_re_client` yakconfig section.\n",
)
edit(
    "app/buck2_interpreter_for_build/src/interpreter/functions/sha1.rs",
    '/// sha1("yak is the best build system") == "d39e9f9030da819a5be667a409ea979551df6211"',
    '/// sha1("yak is the best build system") == "1b40ae7a066087afaae7147598b6d4d7439a685e"',
)
edit(
    "app/buck2_interpreter_for_build/src/interpreter/functions/sha256.rs",
    '/// sha256("yak is the best build system") == "bb99a3f19ecba6c4d2c7cd321b63b669684c713881baae21a6b1d759b3ec6ac9"',
    '/// sha256("yak is the best build system") == "2d42784f6132dd8329ac739b188097943893e333055a94e2ca33eb1ba9421ae2"',
)

# The slice name no longer serves Meta's BPFJailer.
edit(
    "app/buck2_resource_control/src/spawn_daemon.rs",
    """    // N.B. the slice name here is used by BPFJailer to assign the `buck` Role
    // ID to the daemon process, which is what gives the daemon permission to
    // exit the jail.
""",
    "",
)

# The test executor talks to the `TestOrchestrator` trait.
edit(
    "app/buck2_test_api/src/protocol.rs",
    """//! Test executors are expected to implement the trait `TestExecutor`. yak will need to implement
//! the trait `Buck` for the test executor to interact against.
""",
    """//! Test executors are expected to implement the trait `TestExecutor`. yak implements
//! the trait `TestOrchestrator` for the test executor to interact against.
""",
)

# The integration tests keep the `buck` fixture and its `Buck` type.
edit(
    "tests/e2e_util/buck_workspace.py",
    '"Test method must be decorated with @buck_test() to use the yak fixture."',
    '"Test method must be decorated with @buck_test() to use the buck fixture."',
)
edit(
    "tests/core/build/test_final_materializer.py",
    "        buck: yak instance\n",
    "        buck: Buck instance\n",
)
edit(
    "tests/e2e_util/api/buck_result.py",
    "Returns a BuildReport object for a buck v2 build invoked with --build-report.",
    "Returns a BuildReport object for a yak build invoked with --build-report.",
)
# The test compares the output with the fixture, which keeps its strings.
edit(
    "tests/core/run/test_universe_data/YAK.fixture",
    """"DEFAULT": 'print("hello buck")',""",
    """"DEFAULT": 'print("hello yak")',""",
)

# The GUID of a generated solution comes from this string, so it keeps its
# value.
edit(
    "prelude/ide_integrations/visual_studio/gen_sln.bxl",
    'solution_guid = gen_guid("yak vsgo solution file")',
    'solution_guid = gen_guid("Buck2 vsgo solution file")',
)

# The example worker and its test.
edit(
    "examples/persistent_worker/persistent_worker.py",
    'print("BUCK2", request, file=sys.stderr)',
    'print("YAK", request, file=sys.stderr)',
)
edit(
    "examples/persistent_worker/persistent_worker.py",
    'print("BUCK2 WORKER START", file=sys.stderr)',
    'print("YAK WORKER START", file=sys.stderr)',
)
edit(
    "examples/persistent_worker/test.sh",
    'echo "# Verifying Buck2 log" >&2',
    'echo "# Verifying yak log" >&2',
    count=4,
)
edit(
    "examples/persistent_worker/test.sh",
    '(.std_err | startswith("Buck2 persistent worker"))',
    '(.std_err | startswith("yak persistent worker"))',
)

# Files that the rules do not parse.
edit(
    "explorer/index.html",
    "    <title>Buck2 Explorer</title>\n",
    "    <title>yak Explorer</title>\n",
)
edit(
    "explorer/index.html",
    '<button id="buckdir" title="Buck2 directory: XXX">...</button>',
    '<button id="buckdir" title="yak directory: XXX">...</button>',
)
edit(
    "explorer/index.html",
    "Welcome to Buck2 Explorer - enter a target and click a tab to see what <tt>buck2</tt> thinks.",
    "Welcome to yak Explorer - enter a target and click a tab to see what <tt>yak</tt> thinks.",
)
edit("explorer/main.js", 'title: "Buck2 Explorer",', 'title: "yak Explorer",')
edit(
    "explorer/main.js",
    'message: "Select your Buck2 working directory",',
    'message: "Select your yak working directory",',
)
edit("explorer/renderer.js", '$buckdir.title = "Buck2 directory: " + dir;', '$buckdir.title = "yak directory: " + dir;')
edit("explorer/package.json", '"description": "Buck2 Explorer",', '"description": "yak Explorer",')
edit("explorer/package.json", '"name": "Buck2 Explorer",', '"name": "yak Explorer",')
edit(
    "flake.nix",
    'description = "A flake for hacking on and building buck2";',
    'description = "A flake for hacking on and building yak";',
)
edit(
    "integrations/resources/rust/Cargo.toml",
    'description = "Load resource paths from a resources.json produced by Buck"',
    'description = "Load resource paths from a resources.json produced by yak"',
)
edit(
    "prelude/erlang/common_test/test_cli_lib/src/test.erl",
    'io:format("Buck2 Common Test Runner Shell Interface~n~n"),',
    'io:format("yak Common Test Runner Shell Interface~n~n"),',
)
edit(
    "prelude/cxx/tools/serialized_diagnostics_to_json_wrapper.sh",
    "# for Buck error handler consumption. Usage:\n",
    "# for yak error handler consumption. Usage:\n",
)
for path in ("docs/developers/perf/scripts/measure.sh", "docs/developers/perf/scripts/peak_watch.sh"):
    edit(path, "<buck2 args...>", "<yak args...>", count=2)
edit(
    "docs/developers/perf/scripts/bin_waste.py",
    "  bin_waste.py --daemon --buck2 ./yak --isolation-dir v2",
    "  bin_waste.py --daemon --yak ./yak --isolation-dir v2",
)
edit("docs/developers/perf/scripts/bin_waste.py", "        cmd = [args.buck2]", "        cmd = [args.yak]")
edit(
    "docs/developers/perf/scripts/bin_waste.py",
    'ap.add_argument("--buck2", default="yak", help="yak binary for --daemon")',
    'ap.add_argument("--yak", default="yak", help="yak binary for --daemon")',
)
git("mv", ".vscode/test_buckconfig.code-snippets", ".vscode/test_yakconfig.code-snippets")
edit(
    ".vscode/test_yakconfig.code-snippets",
    '"Nanoprelude buckconfig for core test": {',
    '"Nanoprelude yakconfig for core test": {',
)
edit(
    ".vscode/test_yakconfig.code-snippets",
    '"description": "Inserts a buckconfig that is suitable for use in a core test with the nanoprelude",',
    '"description": "Inserts a yakconfig that is suitable for use in a core test with the nanoprelude",',
)
edit(".vscode/test_yakconfig.code-snippets", '"Buckconfig for core test": {', '"yakconfig for core test": {')
edit(
    ".vscode/test_yakconfig.code-snippets",
    '"description": "Inserts a buckconfig that is suitable for use in a core test",',
    '"description": "Inserts a yakconfig that is suitable for use in a core test",',
)

# The workflows that build and publish the binaries.
git("mv", ".github/workflows/build_buck2.yml", ".github/workflows/build_yak.yml")
git("mv", ".github/workflows/upload_buck2.yml", ".github/workflows/upload_yak.yml")
for path in (".github/workflows/release.yml", ".github/workflows/upload_yak.yml"):
    edit(path, "uses: ./.github/workflows/build_buck2.yml", "uses: ./.github/workflows/build_yak.yml")
edit(".github/workflows/release.yml", "buck2_version", "yak_version", count=7)
edit(".github/workflows/upload_yak.yml", "buck2_version", "yak_version", count=6)
bw = ".github/workflows/build_yak.yml"
edit(bw, "buck2_version", "yak_version", count=2)
edit(
    bw,
    """          # BUCK2 prefixed variables used within the build to stamp in a version information
          # YAK_SET_EXPLICIT_VERSION will populate the yak --version string
""",
    """          # YAK_SET_EXPLICIT_VERSION sets the version that yak --version prints.
""",
)
edit(bw, "prefix-key: buck2-upload", "prefix-key: yak-upload")
edit(bw, 'BUCK2="$(pwd)/${{ steps.set_variables.outputs.buck2_out }}"', 'YAK="$(pwd)/${{ steps.set_variables.outputs.yak_out }}"')
edit(bw, '"$BUCK2" build $TARGETS -v=2', '"$YAK" build $TARGETS -v=2')
edit(bw, 'echo "buck2_', 'echo "yak_', count=8)
edit(bw, "steps.set_variables.outputs.buck2_", "steps.set_variables.outputs.yak_", count=4)
uw = ".github/workflows/upload_yak.yml"
edit(uw, "    - name: Download Buck2\n", "    - name: Download yak\n")
edit(uw, "    - name: Decompress Buck2\n", "    - name: Decompress yak\n")
for path in ("ARCHITECTURE.md", "website/README.md"):
    edit(path, ".github/workflows/upload_buck2.yml", ".github/workflows/upload_yak.yml")
edit("docs/exec-plans/tech-debt-tracker.md", ".github/workflows/upload_buck2.yml", ".github/workflows/upload_yak.yml", count=2)

# Comments that explained behavior by comparison with Buck1 state the behavior.
edit(
    "prelude/apple/apple_binary.bzl",
    "        # But we need it for now to achieve compatibility with BUCK1.\n",
    "        # But targets still set the `bridging_header` attribute.\n",
)
edit(
    "prelude/apple/swift/swift_pcm_compilation.bzl",
    """        # but for the sake of BUCK1 compatibility, we need to pass them up,
        # in case they re-export some dependencies.
""",
    """        # but we need to pass them up, in case they re-export some dependencies.
""",
)
edit(
    "app/buck2_build_api/src/interpreter/rule_defs/provider/builtin/external_runner_test_info.rs",
    """/// Provider that signals that a rule can be tested using an external runner. This is the
/// Buck1-compatible API for tests.
""",
    """/// Provider that signals that a rule can be tested using an external runner.
""",
)
edit(
    "app/buck2_client/src/commands/targets.rs",
    "// Use non-camel case so the possible values match buck1's\n",
    "// The possible values are snake case, such as `paths_only`.\n",
)
edit(
    "app/buck2_cmd_audit_server/src/includes.rs",
    """    // To match buck1, if the path is absolute we use it as-is, but if not it is treated
    // as relative to the working dir cell root (not the working dir).
""",
    """    // An absolute path is used as-is, and a relative path is treated as relative to
    // the working dir cell root (not the working dir).
""",
)
edit(
    "app/buck2_cmd_audit_server/src/includes.rs",
    """                    // buck1 has a bug where it doesn't properly handle >1 arg when passed --json
                    // it also, sadly, prints just a single list of outputs for that case. we match
                    // buck1's behavior for 1 successful file and print a dictionary for multiple. This is
                    // unfortunate, but we hope that users can migrate to the equivalent query commands instead.
""",
    """                    // For one successful file, print the list of its includes. For multiple files,
                    // print a dictionary. We hope that users can migrate to the equivalent query
                    // commands instead.
""",
)
edit(
    "app/buck2_cmd_audit_server/src/includes.rs",
    "                                    // To match buck1, we print absolute paths.\n",
    "                                    // Includes are printed as absolute paths.\n",
)
edit(
    "app/buck2_cmd_query_server/src/dot.rs",
    "//! but it's easier for us to match buck1's output with this simple implementation.\n",
    "//! but this simple implementation gives us control over the exact output.\n",
)
edit(
    "app/buck2_cmd_query_server/src/query/printer.rs",
    "            // following buck1's behavior, if any attributes are requested we use json output instead of list output\n",
    "            // If any attributes are requested, we use json output instead of list output.\n",
)
edit(
    "app/buck2_cmd_query_server/src/query/printer.rs",
    """            // TODO(cjhopman): buck1 does this really odd thing that a multi-query that requests any attributes
            // gets the entire result merged together rather than printed as a multi-query. We match that behavior, but
            // it really doesn't make sense and we should migrate off of that.
""",
    """            // TODO(cjhopman): A multi-query that requests any attributes gets the entire result merged
            // together rather than printed as a multi-query. That doesn't make sense, and we should
            // migrate off of it.
""",
)
edit(
    "app/buck2_common/src/ignores/ignore_set.rs",
    "we've done this same ignore (watchman, buck1's ignores).",
    "we've done this same ignore (watchman).",
)
edit(
    "app/buck2_common/src/invocation_roots.rs",
    """/// This is broken when multiple users try to share the same checkout.
///
/// **This is different than the behavior of buck1.**
///
/// In buck1, the yak daemon is shared across users. Due to the fact that `yak run`
/// will run whatever command is returned by the daemon, buck1 has a privilege escalation
/// vulnerability.
///
""",
    """/// This is broken when multiple users try to share the same checkout.
///
/// A daemon shared across users would be a privilege escalation vulnerability, because
/// `yak run` runs whatever command the daemon returns.
///
""",
)
edit(
    "app/buck2_common/src/legacy_configs.rs",
    """//! Contains utilities for dealing with buckv1 concepts (ex. buckv1's
//! .yakconfig files as configuration)
""",
    """//! Contains utilities for reading .yakconfig files as configuration.
""",
)
edit(
    "app/buck2_common/src/legacy_configs/cells.rs",
    "/// Used for creating a CellResolver in a buckv1-compatible way based on values\n",
    "/// Used for creating a CellResolver based on values\n",
)
edit(
    "app/buck2_common/src/pattern/package_roots.rs",
    "                // TODO(cjhopman): Ignoring this matches buck1 behavior, but we'd like this to be an error.\n",
    "                // TODO(cjhopman): This is ignored, but we'd like this to be an error.\n",
)
edit(
    "app/buck2_configured/src/configuration.rs",
    "    // Cell used for yakconfigs is set to cell of target that applies select to match Buck v1 behavior.\n",
    "    // Cell used for yakconfigs is set to cell of target that applies select.\n",
)
edit(
    "app/buck2_configured/src/target_platform_resolution.rs",
    "    // TODO(cjhopman): This needs to implement buck1's approach to determining target platform, it's currently missing the fallback to yakconfig parser.target_platform.\n",
    "    // TODO(cjhopman): This is missing the fallback to yakconfig parser.target_platform.\n",
)
edit(
    "app/buck2_core/src/pattern/pattern.rs",
    """    // Imported from Buck1
    static ALIAS_REGEX""",
    """    static ALIAS_REGEX""",
)
edit(
    "app/buck2_downward_api/src/lib.rs",
    """    async fn external(&self, data: BuckMutMap<String, String>) -> buck2_error::Result<()>;

    // TODO map the StepEvent and TraceEvents in buckv1 to something. Maybe just a single trace event
}""",
    """    async fn external(&self, data: BuckMutMap<String, String>) -> buck2_error::Result<()>;
}""",
)
edit(
    "app/buck2_downward_api_proto/src/lib.rs",
    """//! Protobufs for ineteracting with yak's DownwardApi over GPRC. This isn't the protocol Buck v1
//! speaks, where the DownwardApi is accessed over named pipes with serialized JSON payloads. This
//! is a different way to make the same calls.
""",
    """//! Protobufs for interacting with yak's DownwardApi over gRPC.
""",
)
edit(
    "app/buck2_external_cells_bundled/YAK",
    "    # thing, but that needs to be moved to the prelude after buck1-death. It\n",
    "    # thing, but that needs to be moved to the prelude first. It\n",
)
edit(
    "app/buck2_file_watcher/src/watchman/core.rs",
    """// We use the "new" field. This is marked as deprecated, but buck1 uses it and
// I'm unaware of issues due to its use there.
""",
    """// We use the "new" field. This is marked as deprecated, but I'm unaware of issues due to its
// use.
""",
)
edit(
    "app/buck2_interpreter_for_build_tests/src/functions/host_info.rs",
    """                assert_eq(False, hasattr(host_info(), "buck2"))
                assert_eq(False, hasattr(host_info(), "buck1"))
""",
    """                assert_eq(False, hasattr(host_info(), "buck2"))
""",
)
edit(
    "app/buck2_interpreter_for_build_tests/src/select.rs",
    """            // Adapted from Buck1's `select_introspection` parser test data.
            r#\"""",
    """            r#\"""",
)
edit(
    "app/buck2_node/src/attrs/attr_type/arg.rs",
    """    /// us to progress further into a build and detect more issues. Once we have all (or most) of the buckv1 macros
    /// recognized we'll remove this and make it an early error.
""",
    """    /// us to progress further into a build and detect more issues. Once we recognize all (or most) macros,
    /// we'll remove this and make it an early error.
""",
)
edit(
    "app/buck2_node/src/attrs/attr_type/arg/parser.rs",
    """//! Handles parsing macros out of an attrs.arg()
//!
//! Much of this behavior is inherited from buckv1, which is documented
//! here <https://buck.build/function/string_parameter_macros.html> and here
//! <https://github.com/facebook/buck/blob/5bc82b7c90f1a5c5ac70e2de7d2c2170c289ee79/src/com/facebook/buck/core/macros/MacroFinderAutomaton.java>
//!
""",
    """//! Handles parsing macros out of an attrs.arg()
//!
""",
)
edit(
    "app/buck2_node/src/attrs/attr_type/arg/parser.rs",
    """//! A macro type must be non-empty and consists of characters in `[a-zA-Z0-9_]` followed by whitespace or the
//! macro-ending ')'.
""",
    """//! A macro type must be non-empty and consists of characters in `[a-zA-Z0-9_-]` followed by whitespace or the
//! macro-ending ')'.
""",
)
edit(
    "app/buck2_node/src/attrs/attr_type/arg/parser.rs",
    """//! We diverge from buckv1 in a handful of known ways.
//!
//! 1. buck1 allows pretty much any characters to appear in a macro type. We restrict it to alphanumeric and `_`.
//!
//! 2. buck1 disallows spaces entirely within unquoted args. This can be surprising. Unquoted args are generally used for
//!    the query part of query macros, and in other contexts where yak accepts queries it allows whitespace.
//!    Example, the string "$(query_outputs deps(//some:target, 3))" would be rejected by buck1 due to the space before the 3.
//!
""",
    """//! Unquoted args are generally used for the query part of query macros, so they allow whitespace inside parens, as
//! other contexts where yak accepts queries do. For example, the string "$(query_outputs deps(//some:target, 3))" has
//! one arg with a space before the 3.
//!
""",
)
edit(
    "app/buck2_node/src/attrs/attr_type/arg/parser.rs",
    """// We diverge slightly from buckv1 here.

fn consume_whitespace""",
    """fn consume_whitespace""",
)
edit(
    "app/buck2_node/src/attrs/attr_type/arg/parser.rs",
    "// TODO: that second case seems like a bug in buckv1 and it should be just a single arg. We've preserved the v1 behavior.\n",
    "// TODO: that second case seems like a bug and it should be just a single arg.\n",
)
edit(
    "app/buck2_node/src/attrs/attr_type/arg/parser.rs",
    "// This is much stricter than buckv1. v1 allows nearly any character in the macro type. We allow only alphanumeric, '-', and '_'.\n",
    "// A macro type allows only alphanumeric characters, '-', and '_'.\n",
)
edit(
    "app/buck2_node/src/attrs/spec/internal.rs",
    """/// buck1 used "compatible_with" for this. in yak, we have two "compatible with" concepts, both
/// target and exec compatibility and so we are switching to "target_compatible_with". For now we'll accept
/// either form for target compatibility (but not both).
""",
    """/// yak has two "compatible with" concepts, target and exec compatibility, so this attribute is
/// "target_compatible_with". For now we'll accept "compatible_with" as well for target compatibility
/// (but not both).
""",
)
edit(
    "app/buck2_query/src/query/syntax/simple/eval/values.rs",
    """        // For unbounded traversals, buck1 recommends specifying a large value. We'll accept either a negative (like -1) or
        // a large value as unbounded.""",
    """        // For unbounded traversals, we accept either a negative (like -1) or
        // a large value as unbounded.""",
)
edit(
    "app/buck2_query/src/query/syntax/simple/functions.rs",
    """        // Special-case String + String to match buck1 behavior:
        // treat both strings as target literals in a single evaluation.
""",
    """        // Special-case String + String, which treats both strings as target literals in a single
        // evaluation.
""",
)
edit(
    "app/buck2_query_parser/src/multi_query.rs",
    """            // Unfortunately Buck1 just substitutes in arbitrarily strings, where the query
            // or query_args may not form anything remotely valid.
            // We have to be backwards compatible :(
""",
    """            // Unfortunately the substitution takes arbitrary strings, where the query
            // or query_args may not form anything remotely valid.
""",
)
edit(
    "app/buck2_test_proto/test.proto",
    """// A spec representing test targets aiming for maximum compatibility with the
// external runner spec as defined in buck1.
""",
    """// A spec representing test targets for an external test runner.
""",
)
edit(
    "integrations/resources/rust/src/lib.rs",
    """        "Failed to read manifest file: `{manifest_path}`. \\
        Are you maybe running `buck1`? `rust_binary` only supports `resources` under `yak`!"
""",
    """        "Failed to read manifest file: `{manifest_path}`. \\
        `rust_binary` only supports `resources` under `yak`!"
""",
)
edit(
    "prelude/apple/swift/swift_compilation.bzl",
    """    # We pass through the SDK swiftmodules here to match Buck 1 behaviour. This is
    # pretty loose, but it matches Buck 1 behavior so cannot be improved until
    # migration is complete.
""",
    """    # We pass through the SDK swiftmodules here. This is pretty loose.
""",
)
edit(
    "prelude/apple/tools/bundling/assemble_bundle.py",
    "    2) bundling result has the same structure as in Buck1 even when there are multiple conflicting destination paths\n",
    "    2) bundling result has the same structure as a non-incremental build even when there are multiple conflicting destination paths\n",
)
edit(
    "prelude/apple/tools/bundling/main.py",
    "    # Force same sorting as in Buck1 for `SourcePathWithAppleBundleDestination`\n",
    "    # Sort the spec deterministically.\n",
)
for path, old in (
    (
        "prelude/apple/tools/code_signing/prepare_code_signing_entitlements.py",
        "# Buck v1 corresponding code is in `ProvisioningProfileCopyStep::execute` in `ProvisioningProfileCopyStep.java`\n",
    ),
    (
        "prelude/apple/tools/code_signing/prepare_info_plist.py",
        "# Buck v1 corresponding code is in `ProvisioningProfileCopyStep::execute` in `ProvisioningProfileCopyStep.java`\n",
    ),
    (
        "prelude/apple/tools/code_signing/prepare_info_plist.py",
        "# Equivalent Buck v1 code is in `ProvisioningProfileCopyStep.java` in `ProvisioningProfileCopyStep::getInfoPlistAdditionalKeys` method.\n",
    ),
    (
        "prelude/apple/tools/code_signing/provisioning_profile_metadata.py",
        "    # See `ProvisioningProfileMetadataFactory::getAppIDFromEntitlements` from `ProvisioningProfileMetadataFactory.java` in Buck v1\n",
    ),
    (
        "prelude/apple/tools/code_signing/provisioning_profile_metadata.py",
        "    # See `ProvisioningProfileMetadata::getMergeableEntitlements` from `ProvisioningProfileMetadata.java` in Buck v1\n",
    ),
    (
        "prelude/apple/tools/code_signing/provisioning_profile_metadata.py",
        "    # See `ProvisioningProfileMetadataFactory::fromProvisioningProfilePath` from `ProvisioningProfileMetadataFactory.java` in Buck v1\n",
    ),
    (
        "prelude/apple/tools/code_signing/provisioning_profile_selection.py",
        "# See `ProvisioningProfileStore::getBestProvisioningProfile` in `ProvisioningProfileStore.java` for Buck v1 equivalent\n",
    ),
    (
        "prelude/apple/tools/code_signing/read_provisioning_profile_command_factory.py",
        "# See `DEFAULT_READ_COMMAND` in `AppleConfig.java` in Buck v1\n",
    ),
):
    edit(path, old, "")
edit(
    "prelude/apple/tools/dry_codesign_tool.py",
    """            Tool which implements `DryCodeSignStep` class from buck1.
            Instead of code signing the bundle it just creates a file named `BUCK_code_sign_args.plist` inside,
""",
    """            Instead of code signing the bundle, the tool creates a file named `BUCK_code_sign_args.plist` inside,
""",
)
edit(
    "prelude/apple/tools/dry_codesign_tool.py",
    "        # This is always empty string if you check `DryCodeSignStep` class usages in buck1\n",
    "        # This is always an empty string.\n",
)
edit(
    "prelude/apple/tools/dry_codesign_tool.py",
    "        # Do not sort to keep the ordering same as in buck1.\n",
    "        # Do not sort, so the keys keep their insertion order.\n",
)
edit(
    "prelude/apple/tools/info_plist_processor/main.py",
    """description="Sub-command to expand macro variables in parametrized Info.plist files. It's the Buck v2 equivalent of what `FindAndReplaceStep` and `InfoPlistSubstitution` do.",""",
    """description="Sub-command to expand macro variables in parametrized Info.plist files.",""",
)
edit(
    "prelude/apple/tools/info_plist_processor/main.py",
    """description="Sub-command to do the final processing of the Info.plist before it's copied to the application bundle. It's the Buck v2 equivalent of what `PlistProcessStep` does in v1.",""",
    """description="Sub-command to do the final processing of the Info.plist before it's copied to the application bundle.",""",
)
edit(
    "prelude/apple/tools/info_plist_processor/main.py",
    """description="Tool to process Info.plist file before it is placed into the bundle. It's the Buck v2 equivalent of what `AppleInfoPlist` build rule from v1 does.\"""",
    """description="Tool to process Info.plist file before it is placed into the bundle.\"""",
)
edit(
    "prelude/csharp/csharp.bzl",
    "    # embedded platforms such as Silverlight or WASM. (Originally for Buck1 compatibility.)\n",
    "    # embedded platforms such as Silverlight or WASM.\n",
)
edit(
    "prelude/cxx/archive.bzl",
    "    # sources of non-determinism. See `ObjectFileScrubbers.createDateUidGidScrubber()` in Buck v1.\n",
    "    # sources of non-determinism.\n",
)
edit(
    "prelude/cxx/attr_selection.bzl",
    """    # so write a function that can do either
    #
    # === Buck v1 Compatibility ===
    #
    # `lang_compiler_flags` keys are coerced to CxxSource,
    # so the allowable values are the lowercase versions of the enum values.
    #
    # The keys themselves should be the _output_ type of the language. For example,
    # for Obj-C, that would be OBJC_CPP_OUTPUT.
    #
    # The actual lookup for `lang_compiler_flags` happens in
    # CxxSourceRuleFactory::getRuleCompileFlags().
    #
""",
    """    # so write a function that can do either
    #
    # `lang_compiler_flags` keys are coerced to CxxSource,
    # so the allowable values are the lowercase versions of the enum values.
    #
    # The keys themselves should be the _output_ type of the language. For example,
    # for Obj-C, that would be OBJC_CPP_OUTPUT.
    #
""",
)
edit(
    "prelude/cxx/compile.bzl",
    """    # TODO: Buck v1 validates *all* headers used by a compilation
    # at compile time, but that doing that here/eagerly might be expensive (but
    # we should figure out something).
""",
    """    # TODO: Validate *all* headers used by a compilation
    # at compile time. Doing that here/eagerly might be expensive (but
    # we should figure out something).
""",
)
edit(
    "prelude/cxx/compile.bzl",
    "            # ctx.attrs.compiler_flags need to come last to preserve buck1 ordering, this prevents compiler\n",
    "            # ctx.attrs.compiler_flags need to come last, which prevents compiler\n",
)
edit(
    "prelude/cxx/cxx_toolchain_types.bzl",
    "    # We don't support these buck1 placeholders since we can't take an argument.\n",
    "    # We don't support these placeholders since we can't take an argument.\n",
)
edit(
    "prelude/cxx/omnibus.bzl",
    """    # TODO: This is ported from Buck1, but it might make sense
    # to explicitly tell yak about the global symbols""",
    """    # TODO: It might make sense
    # to explicitly tell yak about the global symbols""",
)
edit(
    "prelude/cxx/preprocessor.bzl",
    """    # The macro $(cxx-header-tree) is used in exactly once place, and its a place which isn't very
    # Buck v2 compatible.""",
    """    # The macro $(cxx-header-tree) is used in exactly once place, and its a place which isn't very
    # compatible with yak.""",
)
edit(
    "prelude/cxx/symbols.bzl",
    "            # Strip off ABI Version (@...) when using llvm-nm to keep compat with buck1\n",
    "            # Strip off ABI Version (@...) when using llvm-nm\n",
)
edit(
    "prelude/decls/core_rules.bzl",
    """            # Buck1 query treated the `tests` attribute of test_suite as deps, and yak query does not.
            # `test_deps` is a deps attribute, so a macro that sets `test_deps = tests` gets the Buck1 query behavior.
""",
    """            # yak query does not treat the `tests` attribute of test_suite as deps.
            # `test_deps` is a deps attribute, so a macro that sets `test_deps = tests` makes query follow the tests.
""",
)
edit(
    "prelude/decls/rust_rules.bzl",
    "        # linker_flags weren't supported for rust_library in Buck v1 but some\n",
    "        # rust_library does not use linker_flags, but some\n",
)
edit(
    "prelude/filegroup.bzl",
    "    # It seems that buck1 always copies, and that's important for Python rules\n",
    "    # Copying is important for Python rules\n",
)
edit(
    "prelude/genrule.bzl",
    """# In Buck1 the SRCS environment variable is only set if the substring SRCS is on the command line.
# That's a horrible heuristic, and doesn't account for users accessing $SRCS from a shell script.
# But in some cases, $SRCS is so large it breaks the process limit, so have a label to opt in to
# that behavior.
""",
    """# In some cases, $SRCS is so large it breaks the process limit, so have a label that leaves the
# SRCS environment variable unset.
""",
)
edit(
    "prelude/genrule.bzl",
    """    # Buck1 uses `.` as output, but that won't work since
    # yak clears the output directory before execution, and thus src/sh too.
""",
    """    # `.` can't be the output, because yak clears the output directory before
    # execution, and thus src/sh too.
""",
)
edit(
    "prelude/haskell/haskell_ghci.bzl",
    """    # NOTE: In the buck1 version we'd symlink the binary only if a custom one
    # was provided, but in yak we're always setting `ghci_bin_dep` (i.e.
    # to default one if custom wasn't provided).
""",
    """    # NOTE: `ghci_bin_dep` is always set (to the default one if a custom one
    # wasn't provided), so the binary is always symlinked.
""",
)
edit(
    "prelude/http_archive/extract_archive.bzl",
    "# Buck v2 doesn't support directories as source inputs, while v1 allows that.\n",
    "# yak doesn't support directories as source inputs.\n",
)
edit(
    "prelude/sh_binary.bzl",
    """    # TODO(cjhopman): Reject cross-repo resources. Buck1 does that. It's probably
    # easier for us (compared to v1) to construct a scheme for them that is
    # correct, but not necessary yet.
""",
    """    # TODO(cjhopman): Reject cross-repo resources. It's probably possible to
    # construct a scheme for them that is correct, but not necessary yet.
""",
)
edit(
    "prelude/sh_binary.bzl",
    """    # This is much, much simpler than the buck1 sh_binary template. A couple reasons:
    # 1. we don't invoke the script through a symlink and so don't need to use and implement a cross-platform `readlink -e`
    # 2. we don't construct an invocation-specific sandbox. The implementation of
    # that in buck1 is pretty crazy and it shouldn't actually be necessary.
    # 3. we don't construct the cell symlinks. those were also strange. They were
    # used for the links in the invocation-specific sandbox (so things would
    # point through the cell symlinks to their original locations). Instead we
    # construct links directly to things (which buck1 actually also did for its
    # YAK_DEFAULT_RUNTIME_RESOURCES).
""",
    """    # The script stays simple:
    # 1. we don't invoke the script through a symlink and so don't need to use and implement a cross-platform `readlink -e`
    # 2. we don't construct an invocation-specific sandbox.
    # 3. we don't construct cell symlinks. Instead we construct links directly to things.
""",
)
for path in (
    "tests/core/audit/test_audit_providers_data/sorted/defs.bzl",
    "tests/core/interpreter/test_v2_only_data/defs.bzl",
):
    edit(path, "# This bzl file cannot be interpreted with Buck1 because there's no `rule` builtin.\n", "")
edit(
    "tests/core/query/cquery/test_filter.py",
    "    # `filter()` function checks unconfigured target label, as Buck1 does.\n",
    "    # `filter()` function checks unconfigured target label.\n",
)
edit(
    "tests/core/query/uquery/test_uquery.py",
    "    # match buck1's strange handling of multi-query with --output-attribute\n",
    "    # A multi-query with --output-attribute merges its results.\n",
)
edit(
    "tests/core/query/uquery/test_uquery.py",
    "    # We'd really prefer this to be an error, but Buck1 allows it\n",
    "    # We'd really prefer this to be an error\n",
)
edit(
    "tests/core/run/test_run.py",
    '    await f([], ["val", "--", "x"])  # Would work differently in Buck1 (no -- to user)\n',
    '    await f([], ["val", "--", "x"])\n',
)

# Examples that used the removed JVM rules and flavors.
edit(
    "prelude/decls/common.bzl",
    """      "attrfilter(annotation_processors, com.foo.Processor, deps('//foo:foo'))\"""",
    """      "attrfilter(labels, generated, deps('//foo:foo'))\"""",
)
edit(
    "prelude/decls/core_rules.bzl",
    """        This genrule() uses a Python script to derive a new
         `AndroidManifest.xml` from an
         `AndroidManifest.xml` in the source tree.""",
    """        This genrule() uses a Python script to derive a
         `config.json` from a
         `config.in` template in the source tree.""",
)
edit("prelude/decls/core_rules.bzl", "          name = 'generate_manifest',\n", "          name = 'generate_config',\n")
edit(
    "prelude/decls/core_rules.bzl",
    """            'AndroidManifest.xml',
          ],
          bash = '$(exe //python/android:basic_to_full) ' \\
              '$SRCDIR/AndroidManifest.xml > $OUT',
          cmd_exe = '$(exe //python/android:basic_to_full) ' \\
              '%SRCDIR%\\\\AndroidManifest.xml > %OUT%',
          out = 'AndroidManifest.xml',""",
    """            'config.in',
          ],
          bash = '$(exe //python/config:expand_template) ' \\
              '$SRCDIR/config.in > $OUT',
          cmd_exe = '$(exe //python/config:expand_template) ' \\
              '%SRCDIR%\\\\config.in > %OUT%',
          out = 'config.json',""",
)
edit(
    "prelude/decls/core_rules.bzl",
    """            'AndroidManifest.xml',
          ],
          bash = '$(exe //python/android:basic_to_full) ' \\
              '$SRCDIR/AndroidManifest.xml > $OUT/AndroidManifest.xml',
          cmd_exe = '$(exe //python/android:basic_to_full) ' \\
              '%SRCDIR%\\\\AndroidManifest.xml > %OUT%\\\\AndroidManifest.xml',
          outs =  {
            "manifest": [ "AndroidManifest.xml" ],
          },
          default_outs = [ "AndroidManifest.xml" ],""",
    """            'config.in',
          ],
          bash = '$(exe //python/config:expand_template) ' \\
              '$SRCDIR/config.in > $OUT/config.json',
          cmd_exe = '$(exe //python/config:expand_template) ' \\
              '%SRCDIR%\\\\config.in > %OUT%\\\\config.json',
          outs =  {
            "config": [ "config.json" ],
          },
          default_outs = [ "config.json" ],""",
)
edit(
    "prelude/decls/core_rules.bzl",
    "generate_manifest_with_named_outputs",
    "generate_config_with_named_outputs",
    count=7,
)
edit(
    "prelude/decls/core_rules.bzl",
    "          yak build //:generate_config_with_named_outputs[manifest]\n",
    "          yak build //:generate_config_with_named_outputs[config]\n",
)
edit(
    "prelude/decls/core_rules.bzl",
    '            src = ":generate_config_with_named_outputs[manifest]",\n',
    '            src = ":generate_config_with_named_outputs[config]",\n',
)
edit(
    "prelude/decls/core_rules.bzl",
    '            out = "some_dir_to_copy_to/AndroidManifest.xml",\n',
    '            out = "some_dir_to_copy_to/config.json",\n',
    count=2,
)
edit(
    "prelude/decls/core_rules.bzl",
    """        output "manifest," which happen to be pointing""",
    """        output "config," which happen to be pointing""",
)
edit(
    "prelude/apple/apple_rules_decls.bzl",
    """                A list of dependencies of this bundle as build targets. You can embed application
                 extensions by specifying the extension's bundle target. To include a WatchKit app, append the
                 flavor `#watch` to the target specification. yak will automatically substitute the appropriate
                 platform flavor (either `watchsimulator` or `watchos`) based on the parent.
""",
    """                A list of dependencies of this bundle as build targets. You can embed application
                 extensions by specifying the extension's bundle target.
""",
)

# Comments that name configuration sections by their names before milestone 3.
edit(
    "app/buck2_common/src/init.rs",
    "/// the `hydration` yak settings or their legacy `buck2_hydration` fallbacks.\n",
    "/// the `hydration` yak settings or their legacy `yak_hydration` fallbacks.\n",
)
edit(
    "remote_execution/re_grpc/src/client.rs",
    "/// Contains runtime options for the remote execution client as set under `buck2_re_client`\n",
    "/// Contains runtime options for the remote execution client as set under `yak_re_client`\n",
)

# The query docs show the targets of this repository and the output of the
# queries against its build. A Mermaid diagram replaces the image of the
# allpaths graph, so the generated pages no longer import useBaseUrl.
QUERY_FUNCTIONS = "app/buck2_query/src/query/syntax/simple/functions.rs"
edit(
    QUERY_FUNCTIONS,
    """    /// $ yak uquery "allpaths(//buck2:buck2, //buck2/app/buck2_validation:buck2_validation)" --output-format=dot > result.dot
    /// $ dot -Tpng result.dot -o image.png
    /// ```
    /// produces the following image:
    /// <img src={useBaseUrl('/img/allpaths_example.png')} class='query-example-image'/>
""",
    """    /// $ yak uquery "allpaths(//:yak, //app/buck2_validation:buck2_validation)" --output-format=dot > result.dot
    /// $ dot -Tpng result.dot -o image.png
    /// ```
    /// produces an image of this graph:
    ///
    /// ```mermaid
    /// flowchart TB
    ///   top["root//:yak"] --> bin["root//app/buck2:buck2-bin"]
    ///   bin --> lib["root//app/buck2:buck2"]
    ///   bin --> validation["root//app/buck2_validation:buck2_validation"]
    ///   lib --> daemon["root//app/buck2_daemon:buck2_daemon"]
    ///   lib --> server["root//app/buck2_server:buck2_server"]
    ///   daemon --> server
    ///   server --> validation
    /// ```
""",
)
edit(
    QUERY_FUNCTIONS,
    """    /// $ yak uquery 'somepath(//buck2:buck2, //buck2/app/buck2_node:buck2_node)'
    ///
    /// //buck2:buck2
    /// //buck2/app/buck2:buck2-bin
    /// //buck2/app/buck2_analysis:buck2_analysis
    /// //buck2/app/buck2_node:buck2_node
""",
    """    /// $ yak uquery 'somepath(//:yak, //app/buck2_node:buck2_node)'
    ///
    /// root//:yak
    /// root//app/buck2:buck2-bin
    /// root//app/buck2_analysis:buck2_analysis
    /// root//app/buck2_node:buck2_node
""",
)
edit(
    QUERY_FUNCTIONS,
    """    /// $ yak uquery "attrfilter(deps, '//buck2/app/buck2_validation:buck2_validation', '//...')"
    ///
    /// //buck2/app/buck2:buck2-bin
    /// //buck2/app/buck2_server:buck2_server
    /// //buck2/app/buck2_server:buck2_server-unittest
    /// ```
    /// returns targets that contain `//buck2/app/buck2_validation:buck2_validation` target in their `deps` attribute.
""",
    """    /// $ yak uquery "attrfilter(deps, 'root//app/buck2_validation:buck2_validation', '//...')"
    ///
    /// root//app/buck2:buck2-bin
    /// root//app/buck2_server:buck2_server
    /// ```
    /// returns targets that contain `root//app/buck2_validation:buck2_validation` target in their `deps` attribute.
    /// A label in an attribute includes its cell name, so the value names the `root` cell.
""",
)
edit(
    QUERY_FUNCTIONS,
    """    /// $ yak uquery "attrregexfilter(deps, '.+validation$', '//...')"
    ///
    /// //buck2/app/buck2:buck2-bin
    /// //buck2/app/buck2_server:buck2_server
    /// //buck2/app/buck2_server:buck2_server-unittest
""",
    """    /// $ yak uquery "attrregexfilter(deps, '.+validation$', '//...')"
    ///
    /// root//app/buck2:buck2-bin
    /// root//app/buck2_server:buck2_server
""",
)
edit(
    QUERY_FUNCTIONS,
    """    /// $ yak uquery 'buildfile(//buck2:buck2)'
    ///
    /// buck2/YAK
""",
    """    /// $ yak uquery 'buildfile(//:yak)'
    ///
    /// YAK
""",
)
edit(
    QUERY_FUNCTIONS,
    '    /// $ yak uquery "buildfile(//buck2/app/buck2_action_impl_tests:buck2_action_impl_tests)"\n',
    '    /// $ yak uquery "buildfile(//app/buck2_action_impl_tests:buck2_action_impl_tests)"\n',
)
edit(
    QUERY_FUNCTIONS,
    "    /// both return `buck2/app/buck2_action_impl_tests/YAK`.\n",
    "    /// both return `app/buck2_action_impl_tests/YAK`.\n",
)
edit(
    QUERY_FUNCTIONS,
    """    /// $ yak uquery "rbuildfiles(//buck2/YAK, //buck2/defs.bzl)"
    ///
    /// buck2/defs.bzl
    /// buck2/YAK
""",
    """    /// $ yak uquery "rbuildfiles(//YAK, //defs.bzl)"
    ///
    /// defs.bzl
    /// YAK
""",
)
edit(
    QUERY_FUNCTIONS,
    """    /// $ yak uquery "deps(//buck2:buck2, 1)"
    /// ```
    /// returns all targets that `//buck2:buck2` depends on directly.
""",
    """    /// $ yak uquery "deps(//:yak, 1)"
    /// ```
    /// returns all targets that `//:yak` depends on directly.
""",
)
edit(
    QUERY_FUNCTIONS,
    """    /// $ yak uquery "filter(validation$, //buck2/app/...)"
    ///
    /// //buck2/app/buck2_validation:buck2_validation
    /// ```
    /// returns all targets within `//buck2/app` that have a label with a `validation` suffix.
""",
    """    /// $ yak uquery "filter(validation$, //app/...)"
    ///
    /// root//app/buck2_validation:buck2_validation
    /// ```
    /// returns all targets within `//app` that have a label with a `validation` suffix.
""",
)
edit(
    QUERY_FUNCTIONS,
    """    /// $ yak uquery "inputs(//buck2/dice/...)"
    /// ```
    /// returns the direct inputs for the `//buck2/dice/...` targets.
""",
    """    /// $ yak uquery "inputs(//dice/...)"
    /// ```
    /// returns the direct inputs for the `//dice/...` targets.
""",
)
edit(
    QUERY_FUNCTIONS,
    """    /// $ yak uquery "owner('app/buck2/src/lib.rs')"
    ///
    /// //buck2/app/buck2:buck2-unittest
    /// //buck2/app/buck2:buck2
""",
    """    /// $ yak uquery "owner('app/buck2/src/lib.rs')"
    ///
    /// root//app/buck2:buck2
""",
)
edit(
    QUERY_FUNCTIONS,
    """    /// $ yak uquery "rdeps(//buck2/..., //buck2/dice/dice:dice, 1)"
    /// ```
    /// returns all targets under `//buck2/...` that depend on `//buck2/dice/dice:dice`.
""",
    """    /// $ yak uquery "rdeps(//..., //dice/dice:dice, 1)"
    /// ```
    /// returns all targets under `//...` that depend on `//dice/dice:dice`.
""",
)
# The targets of this repository list no tests.
edit(
    QUERY_FUNCTIONS,
    """    /// $ yak uquery "testsof(set(//buck2/dice/dice:dice //buck2/app/buck2:buck2))"
    ///
    /// //buck2/dice/dice:dice-unittest
    /// //buck2/app/buck2:buck2-unittest
    /// ```
    /// returns the tests associated with both `//buck2/dice/dice:dice` and `//buck2/app/buck2:buck2`.
""",
    """    /// $ yak uquery "testsof(set(//foo:lib //bar:lib))"
    /// ```
    /// returns the targets that `//foo:lib` and `//bar:lib` list in their `tests` attribute.
""",
)
edit(
    QUERY_FUNCTIONS,
    """    /// $ yak uquery "testsof(deps(//buck2/app/buck2:buck2))"
    /// ```
    /// first finds the transitive closure of `//buck2/app/buck2:buck2`,
""",
    """    /// $ yak uquery "testsof(deps(//foo:bin))"
    /// ```
    /// first finds the transitive closure of `//foo:bin`,
""",
)
edit(
    "website/gen_docs.py",
    """            + "---\\n"
            + "\\nimport useBaseUrl from '@docusaurus/useBaseUrl';\\n\\n"
""",
    """            + "---\\n\\n"
""",
)
edit(
    "website/src/css/custom.css",
    """.query-example-image {
  width: 50%;
  margin: 0 auto;
  display: block;
}

""",
    "",
)
git("rm", "-q", "website/static/img/allpaths_example.png")

if failures:
    print("\n".join(failures))
    sys.exit(1)
print("manual edits applied")
