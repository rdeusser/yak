#!/usr/bin/env python3
"""Hand edits of milestones 1 and 2 that follow rename_runtime.py.

Run from the repository root after `rename_runtime.py --apply`:

    python3 path/to/manual_edits.py

Each edit names its file, the exact text it replaces, and how many times that
text occurs, so a rerun on a changed tree fails instead of editing blindly.
"""

import re
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


def resub(path, pattern, repl, count):
    with open(path, encoding="utf-8") as f:
        text = f.read()
    new, n = re.subn(pattern, repl, text)
    if n != count:
        failures.append(f"{path}: expected {count} matches of {pattern!r}, found {n}")
        return
    with open(path, "w", encoding="utf-8") as f:
        f.write(new)


def git_mv(src, dst):
    subprocess.run(["git", "mv", src, dst], check=True)


def patch_string_table_paths(path, old, new, count):
    """Replaces the prefix `old` of each NUL-terminated string in a binary with
    the shorter `new`, and pads each string with NULs so every string keeps its
    offset."""
    assert len(new) <= len(old)
    with open(path, "rb") as f:
        data = bytearray(f.read())
    n = 0
    start = data.find(old)
    while start != -1:
        end = data.index(b"\0", start)
        pad = b"\0" * (len(old) - len(new))
        data[start : end + 1] = new + data[start + len(old) : end] + b"\0" + pad
        n += 1
        start = data.find(old, start + 1)
    if n != count:
        failures.append(f"{path}: expected {count} strings starting with {old!r}, found {n}")
        return
    with open(path, "wb") as f:
        f.write(data)


# The binary and its process names.
edit("app/buck2/Cargo.toml", '[[bin]]\nname = "buck2"', '[[bin]]\nname = "yak"')
edit("app/buck2/src/lib.rs", '    name = "buck2",\n    about(Some(help())),', '    name = "yak",\n    about(Some(help())),')
edit(
    "app/buck2_wrapper_common/src/is_buck2.rs",
    '        OsStr::new("buck2"),\n        OsStr::new("buck2 (deleted)"),',
    '        OsStr::new("yak"),\n        OsStr::new("yak (deleted)"),',
)
edit(
    "app/buck2_client_ctx/src/subscribers/oom.rs",
    r'Killed process (\d+) \(buck2(?:-daemon)?\)',
    r'Killed process (\d+) \(yak(?:-daemon)?\)',
)
edit("app/buck2_client_ctx/src/subscribers/oom.rs", "task=buck2,", "task=yak,")
edit("app/buck2_client_ctx/src/subscribers/oom.rs", "yak.slice/buck2_daemon.scope_sibling", "yak.slice/yak_daemon.scope_sibling")
edit("app/buck2_client_ctx/src/subscribers/oom.rs", "`user.slice/.../buck2_daemon....scope`", "`user.slice/.../yak-daemon....scope`")
edit("app/buck2_client_ctx/src/subscribers/oom.rs", "`system.slice/buck2_daemon....scope`", "`system.slice/yak-daemon....scope`")
edit("app/buck2_client_ctx/src/subscribers/oom.rs", "(buck2-daemon) total-vm", "(yak-daemon) total-vm", count=0)
edit("app/buck2_resource_control/src/buck_cgroup_tree.rs", "buck2.cg", "yak.cg", count=4)
edit("app/buck2_wrapper_common/src/cleanall/stale.rs", 'async_background_command("buck2")', 'async_background_command("yak")')
edit("app/buck2_wrapper_common/src/cleanall/stale.rs", 'OsStr::new("buck2")', 'OsStr::new("yak")')
resub("app/buck2_wrapper_common/src/lib.rs", r'&\["buck2", ', '&["yak", ', 4)
edit("app/buck2_wrapper_common/src/lib.rs", 'name: "buck2".to_owned(),', 'name: "yak".to_owned(),')
edit("app/buck2_wrapper_common/src/lib.rs", 'let fake_buck2 = temp_dir.join("buck2");', 'let fake_buck2 = temp_dir.join("yak");')
edit("remote_execution/re_grpc/src/client.rs", 'tool_name: "buck2".to_owned(),', 'tool_name: "yak".to_owned(),')
edit("app/buck2_log_common/src/chrome_trace.rs", 'categories: vec!["buck2"],', 'categories: vec!["yak"],')
edit("app/buck2_cmd_docs_client/src/help_doc.rs", 'vec!["buck2".to_owned()]', 'vec!["yak".to_owned()]')
edit("app/buck2_concurrency/src/lib.rs", 'display_command: "buck2".to_owned(),', 'display_command: "yak".to_owned(),', count=3)
edit("app/buck2_event_log/src/write.rs", 'vec!["buck2".to_owned()]', 'vec!["yak".to_owned()]', count=2)
edit("app/buck2_cmd_log_client/src/shed/select_events.rs", 'vec!["buck2".to_owned()]', 'vec!["yak".to_owned()]', count=2)
edit("app/buck2_cmd_debug_client/src/daemon_dir.rs", "(`~/.yakd/xxx`)", "(`~/.yak/yakd/xxx`)")

# Shell completion. The wrappers call the function clap generates from the
# command name, and they no longer register completion for a `buck` command.
bash = "app/buck2_cmd_completion_client/src/completion/completion-wrapper.bash"
edit(bash, '"${_BUCK_COMPLETE_BIN:-buck2}"', '"${_BUCK_COMPLETE_BIN:-yak}"')
edit(
    bash,
    "    complete -F __buck2_fix -o nosort -o bashdefault -o default -o nospace buck\n"
    "    complete -F __buck2_fix -o nosort -o bashdefault -o default -o nospace yak\n"
    "else\n"
    "    complete -F __buck2_fix -o bashdefault -o default -o nospace buck\n"
    "    complete -F __buck2_fix -o bashdefault -o default -o nospace yak\n",
    "    complete -F __yak_fix -o nosort -o bashdefault -o default -o nospace yak\n"
    "else\n"
    "    complete -F __yak_fix -o bashdefault -o default -o nospace yak\n",
)
zsh = "app/buck2_cmd_completion_client/src/completion/completion-wrapper.zsh"
edit(zsh, "#compdef yak buck\n", "#compdef yak\n")
edit(zsh, '"${_BUCK_COMPLETE_BIN:-buck2}"', '"${_BUCK_COMPLETE_BIN:-yak}"')
edit(zsh, "compdef __buck2_fix buck yak", "compdef __yak_fix yak")
fish = "app/buck2_cmd_completion_client/src/completion/completion-wrapper.fish"
edit(fish, "complete -c buck -w yak\n", "")
ps1 = "app/buck2_cmd_completion_client/src/completion/completion-wrapper.ps1"
edit(ps1, "else { 'buck2' }", "else { 'yak' }")
edit(ps1, "-CommandName 'buck', 'buck2'", "-CommandName 'yak'")
for shell_file in (bash, zsh, fish):
    resub(shell_file, r"__buck2_", "__yak_", None or len(re.findall(r"__buck2_", open(shell_file).read())))
    resub(shell_file, r"(?<![A-Za-z0-9_])_buck2 ", "_yak ", len(re.findall(r"(?<![A-Za-z0-9_])_buck2 ", open(shell_file).read())))
obash = "app/buck2_cmd_completion_client/src/completion/options-wrapper.bash"
edit(
    obash,
    '\nif [[ "${BASH_VERSINFO[0]}" -eq 4 && "${BASH_VERSINFO[1]}" -ge 4 || "${BASH_VERSINFO[0]}" -gt 4 ]]; then\n'
    "    complete -F _buck2 -o nosort -o bashdefault -o default -o nospace buck\n"
    "else\n"
    "    complete -F _buck2 -o bashdefault -o default -o nospace buck\n"
    "fi\n",
    "",
)
ozsh = "app/buck2_cmd_completion_client/src/completion/options-wrapper.zsh"
edit(ozsh, "#compdef yak buck\n", "#compdef yak\n")
edit(ozsh, "\ncompdef _buck2 buck\n", "")
ofish = "app/buck2_cmd_completion_client/src/completion/options-wrapper.fish"
edit(ofish, "\ncomplete -c buck -w yak\n", "")
ops1 = "app/buck2_cmd_completion_client/src/completion/options-wrapper.ps1"
edit(ops1, "We register it for both `buck` and `yak`.", "We register it for `yak`.")
edit(ops1, "-CommandName 'buck', 'buck2'", "-CommandName 'yak'")
cv = "shed/completion_verify/src/main.rs"
edit(cv, 'default_value = "buck2"', 'default_value = "yak"')
edit(cv, 'run("buck2", script, "yak abcdefghijkl"', 'run("yak", script, "yak abcdefghijkl"')
edit(cv, "`% buck2\\ntargets   test`", "`% yak\\ntargets   test`")
tc = "tests/core/completion/test_completion.py"
edit(tc, '    bin: str = "buck2",', '    bin: str = "yak",')
# Bash completes the build file in target position, which the test shows by
# typing a prefix of its name.
edit(tc, 'input="build TARG",', 'input="build YA",', count=2)
edit(
    tc,
    """completion_test(
    name="test_build_flags_buck_bin",
    # Use `--p` so that we don't get too many outputs, which the test framework doesn't handle well
    # on zsh
    input="build --p",
    options_only=True,
    expected=lambda actual: (
        "--prefer-local" in actual and "--prefer-remote" in actual
    ),
    bin="buck",
)

""",
    "",
)

# Build files: `YAK` is the only default, and `[buildfile] name` lists exact names.
bf = "app/buck2_common/src/buildfiles.rs"
edit(
    bf,
    """const DEFAULT_BUILDFILES: &[&str] = &["BUCK.v2", "YAK"];

/// Deal with the `buildfile.name` key (and `name_v2`)
pub fn parse_buildfile_name(
    mut config: impl LegacyBuckConfigView,
) -> buck2_error::Result<Vec<FileNameBuf>> {
    // For yak, we support a slightly different mechanism for setting the buildfile to
    // assist with easier migration from v1 to v2.
    // First, we check the key `buildfile.name_v2`, if this is provided, we use it.
    // Second, if that wasn't provided, we will use `buildfile.name` like buck1 does,
    // but for every entry `FOO` we will insert a preceding `FOO.v2`.
    // If neither of those is provided, we will use the default of `["BUCK.v2", "YAK"]`.
    // This scheme provides a natural progression to buckv2, with the ability to use separate
    // buildfiles for the two where necessary.
    let mut base = if let Some(buildfiles_value) =
        config.parse_list::<String>(BuckconfigKeyRef {
            section: "buildfile",
            property: "name_v2",
        })? {
        buildfiles_value.into_try_map(FileNameBuf::try_from)?
    } else if let Some(buildfiles_value) = config.parse_list::<String>(BuckconfigKeyRef {
        section: "buildfile",
        property: "name",
    })? {
        let mut buildfiles = Vec::new();
        for buildfile in buildfiles_value {
            buildfiles.push(FileNameBuf::try_from(format!("{buildfile}.v2"))?);
            buildfiles.push(FileNameBuf::try_from(buildfile)?);
        }
        buildfiles
    } else {""",
    """const DEFAULT_BUILDFILES: &[&str] = &["YAK"];

/// parse_buildfile_name returns the build file names of a cell: the list in `buildfile.name`, or
/// `YAK` when the key is unset, followed by `buildfile.extra_for_test` when it is set.
pub fn parse_buildfile_name(
    mut config: impl LegacyBuckConfigView,
) -> buck2_error::Result<Vec<FileNameBuf>> {
    let mut base = if let Some(buildfiles_value) = config.parse_list::<String>(BuckconfigKeyRef {
        section: "buildfile",
        property: "name",
    })? {
        buildfiles_value.into_try_map(FileNameBuf::try_from)?
    } else {""",
)
edit(
    bf,
    """                                other = other/
                                third_party = third_party/
                        "#""",
    """                                other = other/
                        "#""",
)
edit(
    bf,
    """                            [buildfile]
                                name = TARGETS
                                extra_for_test = YAK.test
                        "#
                ),
            ),
            (
                "third_party/.yakconfig",
                indoc!(
                    r#"
                            [cells]
                                third_party = .
                            [buildfile]
                                name_v2 = OKAY
                                name = OKAY_v1
                        "#
                ),
            ),""",
    """                            [buildfile]
                                name = BUILD,YAK
                                extra_for_test = BUILD.test
                        "#
                ),
            ),""",
)
edit(bf, 'vec!["BUCK.v2", "YAK"],', 'vec!["YAK"],')
edit(
    bf,
    """            vec!["TARGETS.v2", "TARGETS", "YAK.test"],
            parse_buildfile_name(&config)?.map(|f| f.as_str()),
        );

        let config = cells
            .parse_single_cell_with_file_ops(CellName::testing_new("third_party"), &mut file_ops)
            .await?;
        assert_eq!(
            vec!["OKAY"],""",
    """            vec!["BUILD", "YAK", "BUILD.test"],""",
)
cl = "app/buck2_common/src/legacy_configs/cells.rs"
edit(cl, "                            [buildfile]\n                                name_v2 = OKAY\n                                name = OKAY_v1", "                            [buildfile]\n                                name = OKAY")
edit(cl, "                            [buildfile]\n                                name = TARGETS\n", "                            [buildfile]\n                                name = BUILD\n", count=2)
pl = "app/buck2_common/src/package_listing/interpreter.rs"
edit(
    pl,
    """                if let Some(primary_candidate) =
                    candidates.iter().find(|v| v.extension() != Some("v2"))
                {
                    let alternatives: Vec<_> = candidates
                        .iter()
                        .filter(|v| *v != primary_candidate)
                        .map(|v| format!("`{v}`"))
                        .collect();
""",
    """                if let Some((primary_candidate, alternatives)) = candidates.split_first() {
                    let alternatives: Vec<_> =
                        alternatives.iter().map(|v| format!("`{v}`")).collect();
""",
)
edit(pl, "missing `TARGETS` file (also missing alternatives `TARGETS.v2`, `YAK`, `BUCK.v2`)", "missing `YAK` file (also missing alternatives `BUILD`)")
edit(
    "app/buck2_cmd_completion_client/src/complete/flagfile.rs",
    'const NON_FLAGFILE_NAMES: &[&str] = &["YAK", "TARGETS", "PACKAGE"];',
    'const NON_FLAGFILE_NAMES: &[&str] = &["YAK", "PACKAGE"];',
)
edit(
    "prelude/YAK",
    "# Tests want BUCK.v2 instead of TARGETS.v2\n",
    "# The glob in `srcs` stops at the `android/constraints` package, so this copies\n# its build file into the prelude.\n",
)
edit(
    "starlark-rust/vscode/package.json",
    '                    "YAK",\n                    "BUCK.v2",\n                    "BUILD_DEFS",\n                    "DEFS",\n                    "TARGETS",\n                    "TARGETS.v2",\n',
    '                    "YAK",\n                    "BUILD_DEFS",\n                    "DEFS",\n',
)
edit(
    "website/docs/rule_authors/writing_rules.md",
    "A common way to test is to use `genrule` to cause the produced binary to run and\n"
    "assert some properties from it. If your rule is in Buck1 and Buck2, use a\n"
    "`YAK` file so you can test with both. If your tests are incompatible with Buck1\n"
    "(such as if it is a new rule), use `BUCK.v2`, which will only be seen by Buck2\n"
    "and won't cause errors with Buck1.\n",
    "A common way to test is to use `genrule` to cause the produced binary to run and\n"
    "assert some properties from it.\n",
)

# The repository's own build: the `//:yak` target and the bundle it runs.
edit("YAK", '    name = "buck2",\n    actual = "//app/buck2:buck2-bin",', '    name = "yak",\n    actual = "//app/buck2:buck2-bin",')
edit("YAK", "# The transition builds buck2 with pagable enabled", "# The transition builds yak with pagable enabled")
edit("YAK", "# client-only build expects. `buck2.py` runs it.", "# client-only build expects. `yak.py` runs it.")
edit("YAK", 'load(":defs.bzl", "buck2_bundle", "pagable_transition_alias")', 'load(":defs.bzl", "pagable_transition_alias", "yak_bundle")')
edit("YAK", 'buck2_bundle(\n    name = "buck2_bundle",', 'yak_bundle(\n    name = "yak_bundle",')
edit("defs.bzl", "def _buck2_bundle_impl(ctx: AnalysisContext)", "def _yak_bundle_impl(ctx: AnalysisContext)")
edit("defs.bzl", "buck2_bundle = rule(\n    impl = _buck2_bundle_impl,", "yak_bundle = rule(\n    impl = _yak_bundle_impl,")
edit("defs.bzl", "Puts the client binary (`buck2`) and the daemon binary (`yak-daemon`)", "Puts the client binary (`yak`) and the daemon binary (`yak-daemon`)")
edit("defs.bzl", 'buck2_binary = "buck2" + binary_extension', 'buck2_binary = "yak" + binary_extension')
edit("defs.bzl", 'out.project("buck2" + binary_extension)', 'out.project("yak" + binary_extension)')
git_mv("buck2.py", "yak.py")
git_mv("buck2.bat", "yak.bat")
edit("yak.bat", '"%~dp0buck2.py"', '"%~dp0yak.py"')
edit("yak.py", "#   ./buck2.py build //my:target", "#   ./yak.py build //my:target")
edit("yak.py", '        "buck2",\n        "run",\n        "//:buck2_bundle",', '        "yak",\n        "run",\n        "//:yak_bundle",')
edit(".github/actions/build_bootstrap/action.yml", "artifacts/buck2 build :buck2 -v 2", "artifacts/yak build :yak -v 2")
edit(".vscode/launch.json", '"name": "Debug buck2",', '"name": "Debug yak",')
edit(".vscode/launch.json", '"name": "buck2",\n                    "kind": "bin"', '"name": "yak",\n                    "kind": "bin"')
edit(".claude/skills/buck2-rule-basics/SKILL.md", "./buck2.py", "./yak.py", count=2)
edit("docs/developers/debugging.md", "`./buck2.py <command>` builds `//:buck2_bundle`", "`./yak.py <command>` builds `//:yak_bundle`")
edit("website/README.md", "./buck2.py", "./yak.py")
edit("website/gen_docs.py", '        return "buck2"\n', '        return "yak"\n')
edit("website/gen_docs.py", '        return "./buck2.py"\n', '        return "./yak.py"\n')

# CI: the binary that `cargo build --bin=yak` writes, and the release assets.
for wf in (
    ".github/workflows/build-and-examples.yml",
    ".github/actions/build_example_conan/action.yml",
    ".github/actions/build_example_no_prelude/action.yml",
    ".github/actions/build_example_toolchain/action.yml",
):
    resub(wf, r"artifacts/buck2(?= )", "artifacts/yak", len(re.findall(r"artifacts/buck2(?= )", open(wf).read())))
bw = ".github/workflows/build_buck2.yml"
edit(bw, "/release/buck2.exe", "/release/yak.exe")
edit(bw, '/release/buck2" >>', '/release/yak" >>')
edit(bw, "artifacts/buck2-${{ matrix.target.triple }}", "artifacts/yak-${{ matrix.target.triple }}", count=2)
edit(bw, "name: buck2-${{ matrix.target.triple }}", "name: yak-${{ matrix.target.triple }}")
uw = ".github/workflows/upload_buck2.yml"
edit(uw, "name: buck2-x86_64-unknown-linux-gnu", "name: yak-x86_64-unknown-linux-gnu")
edit(uw, "artifacts/buck2-x86_64-unknown-linux-gnu.zst -o artifacts/buck2-release", "artifacts/yak-x86_64-unknown-linux-gnu.zst -o artifacts/yak-release")
edit(uw, "chmod +x artifacts/buck2-release", "chmod +x artifacts/yak-release")
edit(uw, 'artifacts/buck2-release" yarn build_prebuilt', 'artifacts/yak-release" yarn build_prebuilt')
dc = ".github/dotslash-config.json"
edit(dc, '      "buck2": {', '      "yak": {')
resub(dc, r'"\^buck2-', '"^yak-', 7)
resub(dc, r'"path": "buck2-', '"path": "yak-', 7)

# Tools and tests that run the binary by name.
for path, n in (
    ("prelude/erlang/shell/src/shell_buck2_utils.erl", 4),
    ("prelude/ide_integrations/visual_studio/vsgo.py", 4),
):
    resub(path, r'(~?)"buck2"', r'\1"yak"', n)
edit("prelude/go/tools/gobuckify/lib/golist.go", '[]string{"buck2", "run"', '[]string{"yak", "run"')
edit("prelude/go/tools/gopackagesdriver/driver/buck.go", "\"running 'buck2'\"", "\"running 'yak'\"")
edit("prelude/go/tools/gopackagesdriver/driver/buck.go", 'b.cmder.Exec(ctx, "buck2", args...)', 'b.cmder.Exec(ctx, "yak", args...)')
edit("prelude/python/sourcedb/tests/pyrefly_test.py", '            "buck2",\n            "bxl",', '            "yak",\n            "bxl",')
edit("prelude/toolchains/conan/conan_update.py", '["buck2", "root"]', '["yak", "root"]')
edit("examples/vscode/.vscode/tasks.json", '"command": "buck2",', '"command": "yak",', count=2)
edit("explorer/main.js", '"--", "buck2"].concat(args)', '"--", "yak"].concat(args)')
edit("integrations/rust-project/src/buck.rs", 'command.unwrap_or_else(|| "buck2".into())', 'command.unwrap_or_else(|| "yak".into())')
edit("integrations/rust-project/src/main.rs", 'Defaults to `"buck2"`.', 'Defaults to `"yak"`.', count=3)
edit("docs/developers/perf/scripts/bin_waste.py", 'ap.add_argument("--buck2", default="buck2", help="yak binary for --daemon")', 'ap.add_argument("--buck2", default="yak", help="yak binary for --daemon")')
edit("tests/e2e_util/buck_workspace.py", 'exe = "yak.exe" if platform.system() == "Windows" else "buck2"', 'exe = "yak.exe" if platform.system() == "Windows" else "yak"')
th = "tests/core/help/test_help.py"
edit(th, 'r"buck2 [a-z0-9]{16,64} (<build-id>|<exe-hash>)",\n        "buck2 <version> <version-source>",', 'r"yak [a-z0-9]{16,64} (<build-id>|<exe-hash>)",\n        "yak <version> <version-source>",')
edit(th, 're.sub(r"buck2\\.exe", "buck2", s)', 're.sub(r"yak\\.exe", "yak", s)')

# Prose that describes the rename, the upstream files, or Buck1.
edit(
    "AGENTS.md",
    "The fork will be renamed to yak, and its default build file name will change from `YAK` to `YAK`. [The rename plan](docs/exec-plans/active/2026-09-28-rename-the-fork.md) tracks the work.",
    "The fork is being renamed to yak. The binary is `yak`, it reads `YAK` build files and `.yakconfig` files, and it writes `yak-out`. [The rename plan](docs/exec-plans/active/2026-09-28-rename-the-fork.md) tracks the remaining work, such as the `BUCK2_` environment variables and the `buck2*` crates.",
)
edit(
    "ARCHITECTURE.md",
    "- The project will be renamed to yak, and the default build file name will change from `YAK` to `YAK`. `docs/exec-plans/active/2026-09-28-rename-the-fork.md` tracks the work.\n"
    "- `DEFAULT_BUILDFILES` in `app/buck2_common/src/buildfiles.rs` sets the default build file names (`BUCK.v2`, `YAK`), and the `[buildfile] name` buckconfig key overrides them for a cell.\n",
    "- The rename to yak continues with the `BUCK2_` environment variables, the `[buck2]` configuration sections, the Java packages, and the `buck2*` crates. `docs/exec-plans/active/2026-09-28-rename-the-fork.md` tracks the work.\n",
)
edit(
    "docs/developers/basics.md",
    "Upstream `YAK` files load macros from Meta's cells and name crates by their path inside Meta's repository. A ported `YAK` file loads",
    "Upstream `BUCK` files load macros from Meta's cells and name crates by their path inside Meta's repository. A ported build file is named `YAK`, loads",
)
edit(
    "app/buck2_common/src/invocation_roots.rs",
    "Couldn't find a buck project root for directory `{}`. Expected to find a .yakconfig file.",
    "Couldn't find a project root for directory `{}`. Expected to find a .yakconfig file.",
)
bop = "app/buck2_cmd_audit_server/src/output/buck_out_path_parser.rs"
edit(
    bop,
    """#[derive(Debug, buck2_error::Error)]
#[buck2(tag = InvalidBuckOutPath)]
enum BuckOutPathParserError {
    #[error(
        "Path does not start with `yak-out`. This is probably a yak-out generated by buck1: `{0}`"
    )]
    MaybeBuck1Path(String),
}

""",
    "",
)
edit(
    bop,
    """    match iter.next() {
        Some(buck_out) => {
            if buck_out != "yak-out" {
                // In buck1, the isolation prefix is prepended to `yak-out` (ex: `.my-isolation-dir-yak-out`).
                // Let's emit a more specific error when this happens.
                if buck_out.as_str().ends_with("yak-out") {
                    return Err(
                        BuckOutPathParserError::MaybeBuck1Path(output_path.to_owned()).into(),
                    );
                } else {
                    return Err(buck2_error!(
                        buck2_error::ErrorTag::Input,
                        "Path does not start with yak-out"
                    ));
                }
            }
        }
        None => {
            return Err(buck2_error!(
                buck2_error::ErrorTag::Input,
                "Path does not start with yak-out"
            ));
        }
    }
""",
    """    match iter.next() {
        Some(buck_out) if buck_out == "yak-out" => {}
        _ => {
            return Err(buck2_error!(
                buck2_error::ErrorTag::Input,
                "Path does not start with yak-out"
            ));
        }
    }
""",
)
edit(
    bop,
    "    iter: &mut Peekable<impl Iterator<Item = &'v FileName>>,\n    output_path: &str,\n) -> buck2_error::Result<()> {",
    "    iter: &mut Peekable<impl Iterator<Item = &'v FileName>>,\n) -> buck2_error::Result<()> {",
)
edit(bop, "validate_buck_out_and_isolation_prefix(&mut iter, output_path)?;", "validate_buck_out_and_isolation_prefix(&mut iter)?;")
edit(bop, '        let buck1_path = ".some-isolation-yak-out/art/bar/path/to/target/__foo__/bar";\n', "")
edit(
    bop,
    """        let res = buck_out_parser.parse(buck1_path);
        assert!(res.err().unwrap().to_string().contains("buck1"));

""",
    "",
)
cfgdoc = "website/docs/concepts/buckconfig.md"
edit(cfgdoc, "3. File `buckconfig` and directory `buckconfig.d` located in system directory", "3. File `yakconfig` and directory `yakconfig.d` located in system directory")
edit(
    cfgdoc,
    "`.yakconfig.d`(`buckconfig.d`) directory (excluding files found in\n"
    "subdirectories) as a Buck2 configuration file, provided that it adheres to\n"
    "`.yakconfig` syntax. Note that a `.yakconfig.d` directory is distinct from the\n"
    "similarly-named `.yakd` directory which is used by the\n"
    "[Buck2 Daemon (`yakd`)](daemon.md) . For a description of how Buck2 resolves\n",
    "`.yakconfig.d`(`yakconfig.d`) directory (excluding files found in\n"
    "subdirectories) as a Buck2 configuration file, provided that it adheres to\n"
    "`.yakconfig` syntax. For a description of how Buck2 resolves\n",
)
edit(cfgdoc, "`.yakconfig.d` (`buckconfig.d`) directory", "`.yakconfig.d` (`yakconfig.d`) directory")
edit("app/buck2_common/src/legacy_configs/configs.rs", "listing files in `buckconfig.d` directories", "listing files in `yakconfig.d` directories")

# Reindeer writes the third-party build file under the name the repository reads.
edit(
    "third-party/rust/reindeer.toml",
    "[buck]\nplatform_compatibility_on_all_targets = true\n",
    '[buck]\nfile_name = "YAK"\nplatform_compatibility_on_all_targets = true\n',
)

# Editors and GitHub highlight `YAK` files as Starlark.
edit(".gitattributes", "PACKAGE linguist-language=Starlark\n", "PACKAGE linguist-language=Starlark\nYAK linguist-language=Starlark\n")
with open(".vscode/settings.json", "w", encoding="utf-8") as f:
    f.write('{\n    "files.associations": {\n        "YAK": "starlark"\n    }\n}\n')
subprocess.run(["git", "add", ".vscode/settings.json"], check=True)

# Examples and comments that name Meta's `TARGETS` build file.
for path, n in (
    ("app/buck2_build_api/src/bxl/unconfigured_attribute.rs", 2),
    ("app/buck2_bxl/src/bxl/starlark_defs/cquery.rs", 1),
    ("app/buck2_bxl/src/bxl/starlark_defs/uquery.rs", 1),
    ("app/buck2_bxl/src/bxl/starlark_defs/nodes/configured.rs", 8),
):
    resub(path, r'(bin|cell//path/to)/TARGETS"', r'\1/YAK"', n)
edit("app/buck2_bxl/src/bxl/starlark_defs/lazy_ctx/lazy_uquery_ctx.rs", 'rbuildfiles("bin/TARGETS", "bin/defs.bzl")', 'rbuildfiles("bin/YAK", "bin/defs.bzl")')
edit("app/buck2_bxl/src/bxl/starlark_defs/lazy_ctx/lazy_uquery_ctx.rs", '["bin/TARGETS", "lib/TARGETS"]', '["bin/YAK", "lib/YAK"]')
edit("app/buck2_core/src/package.rs", "+-- TARGETS\n", "+-- YAK\n", count=4)
edit("app/buck2_interpreter/src/file_type.rs", "a `.bzl` file, `.bxl` file, or a `YAK`/`TARGETS` file.", "a `.bzl` file, `.bxl` file, or a `YAK` file.")
edit("app/buck2_interpreter_for_build/src/plugins.rs", "/// # TARGETS file:", "/// # YAK file:")
edit("app/buck2_node/src/nodes/eval_result.rs", "a build file (YAK, TARGETS, etc) will only be loaded in", "a build file (`YAK` by default) will only be loaded in")
edit("app/buck2_query/src/query/syntax/simple/functions.rs", "`buck2/app/buck2_action_impl_tests/TARGETS`", "`buck2/app/buck2_action_impl_tests/YAK`")
edit("app/buck2_query_impls/src/uquery/environment.rs", "(the YAK/TARGETS file)", "(the build file)")
edit("integrations/rust-project/src/buck.rs", "corresponds to the YAK/TARGETS file of a target.", "corresponds to the build file of a target.")
edit("integrations/rust-project/src/project_json.rs", "`build_file` corresponds to the `YAK`/`TARGETS` file.", "`build_file` corresponds to the build file, such as `YAK`.")
edit("prelude/haskell/ide/ide.bxl", "(e.g. TARGETS files)", "(e.g. YAK files)")
edit("prelude/toolchains/conan/defs.bzl", "Generate a TARGETS file with this name next to the Conanfile.", "Generate a build file with this name next to the Conanfile.")
edit("app/buck2_interpreter_for_build_tests/src/interpreter.rs", 'BuildFilePath::testing_new("root//prelude:TARGETS.v2")', 'BuildFilePath::testing_new("root//prelude:YAK")')
# Buck1 wording, where `BUCK` named the tool.
edit(
    "prelude/decls/android_rules.bzl",
    """        separates an apk\\\\_genrule from a genrule is apk\\\\_genrules are known by YAK to
        produce APKs, so commands like `buck install` or
         `buck uninstall` still work. Additionally,
""",
    """        separates an apk\\\\_genrule from a genrule is apk\\\\_genrules are known by yak to
        produce APKs, so `yak install` still works. Additionally,
""",
)
# The source folder is the directory of the build file, whatever `[buildfile]
# name` calls the file and wherever its cell is. The cell's name matched its
# directory only in a layout like Meta's.
edit(
    "prelude/rust/rust-analyzer/resolve_deps.bxl",
    """        abs_buildfile_path = ctx.root() + "/" + target.buildfile_path.cell + "/" + target.buildfile_path.path
        source_folder = abs_buildfile_path.removesuffix("/TARGETS").removesuffix("/YAK")
""",
    """        abs_buildfile_path = ctx.root() + "/" + ctx.fs.project_rel_path(target.buildfile_path)
        source_folder = abs_buildfile_path.rsplit("/", 1)[0]
""",
)
# Paths that name the binary, which the command rule skips after a `/`.
edit("app/buck2_wrapper_common/src/is_buck2.rs", '("/dir/buck2", "/dir/other")', '("/dir/yak", "/dir/other")')
edit("docs/developers/perf/scripts/bin_waste.py", "bin_waste.py --daemon --buck2 ./buck2 --isolation-dir v2", "bin_waste.py --daemon --buck2 ./yak --isolation-dir v2")
edit("examples/with_prelude/README.md", 'export BUCK2="/tmp/bin/buck2"', 'export BUCK2="/tmp/bin/yak"')
edit("tests/core/build/test_modify.py", '# Change "HELLO" in TARGETS to "GOODBYE"', '# Change "HELLO" in YAK.fixture to "GOODBYE"')
edit("tests/core/ctargets_command/test_ctargets_json_report_data/b/YAK.fixture", "fail during TARGETS file evaluation", "fail during build file evaluation")
edit("tests/core/ctargets_command/test_ctargets_keep_going_data/b/YAK.fixture", "fail during TARGETS file evaluation", "fail during build file evaluation")
edit("tests/core/errors/test_errors.py", "has no TARGETS file", "has no build file")
edit("tests/core/query/test_buildfiles.py", 'assert "transitive_load/TARGETS" in out1', 'assert "transitive_load/YAK" in out1')
edit("website/docs/concepts/glossary.md", "A `YAK` file (the name is configurable, some projects use `TARGETS`) is the", "A `YAK` file (the name is configurable) is the")
edit("website/docs/rfcs/drafts/plugin-deps.md", "### TARGETS\n", "### YAK\n")
sfa = "tools/starlark_fmt/lib/autofixes.rs"
edit(sfa, 'Path::new("TARGETS")', 'Path::new("YAK")')
edit(sfa, 'Path::new("foo/TARGETS")', 'Path::new("foo/YAK")', count=2)
edit(sfa, '"foo/TARGETS:3:21-32"', '"foo/YAK:3:21-32"')
edit("tools/starlark_fmt/lib/config.rs", 'config.sort_flags(Path::new("TARGETS"))', 'config.sort_flags(Path::new("BUILD"))')
edit("tools/starlark_fmt/lib/autofixes/sort_list_args.rs", "every BUILD/TARGETS file", "every build file")
edit("tools/starlark_fmt/tests/test_bzl_skipping.t", "  $ cat <<'EOF' > TARGETS\n", "  $ cat <<'EOF' > YAK\n")
edit("tools/starlark_fmt/tests/test_bzl_skipping.t", "  $ starlark-fmt-cfg TARGETS\n   INFO process_file: TARGETS: formatted\n  $ cat TARGETS\n", "  $ starlark-fmt-cfg YAK\n   INFO process_file: YAK: formatted\n  $ cat YAK\n")

# Completion treats `*.yakconfig` files as configuration, and only `YAK` and
# `PACKAGE` as build files.
ff = "app/buck2_cmd_completion_client/src/complete/flagfile.rs"
edit(ff, 'const NON_FLAGFILE_EXTENSIONS: &[&str] = &[\n    "buckconfig",', 'const NON_FLAGFILE_EXTENSIONS: &[&str] = &[\n    "yakconfig",')
edit(ff, '        assert!(!is_flagfile("TARGETS"));\n', "")

# The reserved directory in `yak-out`, and the isolation dir names it blocks.
edit(
    "app/buck2_common/src/invocation_paths.rs",
    "/// This points to yak-out/._buck2/trash which is used as the trash directory.",
    "/// This points to yak-out/._yak/trash which is used as the trash directory.",
)
edit(
    "app/buck2_common/src/invocation_paths.rs",
    """/// One entry is set aside for tools other than buck2: `yak-out/._buck2/tmp` is scratch space
/// for tooling (e.g. compiler wrappers invoked outside of a yak action) that needs a temp
/// location under yak-out. Buck2 never stores its own state there, and `clean --all` deletes
/// it like any other reserved entry, so contents must be disposable.
pub const RESERVED_BUCK_OUT_PREFIX: &str = "._buck2";""",
    """/// One entry is set aside for tools other than yak: `yak-out/._yak/tmp` is scratch space
/// for tooling (e.g. compiler wrappers invoked outside of a yak action) that needs a temp
/// location under yak-out. yak never stores its own state there, and `clean --all` deletes
/// it like any other reserved entry, so contents must be disposable.
pub const RESERVED_BUCK_OUT_PREFIX: &str = "._yak";""",
)
edit(
    "tests/core/clean/test_clean.py",
    'buck.set_isolation_prefix("._buck2_anything")',
    'buck.set_isolation_prefix("._yak_anything")',
)
edit(
    "tests/core/clean/test_clean.py",
    'stderr_regex="reserved for buck2",',
    'stderr_regex="reserved for yak",',
)

# The selective debugging scrubber recognizes build outputs by their `yak-out/`
# prefix, so the object file paths (N_OSO strings) in its test binary use it.
patch_string_table_paths(
    "prelude/apple/tools/selective_debugging/test_resources/HelloWorld",
    b"buck-out/",
    b"yak-out/",
    count=4,
)

# The changelog lists the new names.
edit(
    "CHANGELOG.md",
    "## Unreleased\n\nRemoves the code, configuration, and service clients that only Meta's internal build used.\n\n",
    """## Unreleased

Removes the code, configuration, and service clients that only Meta's internal build used, and renames the tool to yak.

### Renamed to yak

- The binary is `yak`, and `cargo build --bin=yak` builds it.
- yak reads build files named `YAK`.
- `[buildfile] name` lists the exact build file names to read. The `name_v2` key and the `.v2` variant of each name are gone.
- The project configuration files are `.yakconfig`, `.yakconfig.local`, and `.yakconfig.d/`.
- The global configuration is in `/etc/yakconfig` and `/etc/yakconfig.d/`, or in `C:\\ProgramData\\yakconfig` and `C:\\ProgramData\\yakconfig.d` on Windows.
- A `.yakroot` file marks the project root.
- The settings files are `.yaksettings.toml` and `.yaksettings.local.toml`.
- Build output goes to `yak-out`.
- The reserved directory in `yak-out` is `._yak`. Tools keep scratch files in `yak-out/._yak/tmp`, and `--isolation-dir` rejects names that start with `._yak`.
- The daemon keeps its state in `~/.yak/yakd/`, in files named `yakd.info`, `yakd.pid`, and so on.
- The alternative package file name is `YAK_TREE` in place of `BUCK_TREE`.
- yak reads none of the old names. A project renames its `BUCK`, `.buckconfig`, and `.buckroot` files, or sets `[buildfile] name = BUCK` in `.yakconfig` to keep its build files.
- For a crate whose target is incompatible with the host, `prelude//rust/rust-analyzer/resolve_deps.bxl` reports the directory of its build file as the source folder. It removed only a `/TARGETS` or `/BUCK` file name before. It also joined the cell's name to the project root in place of the cell's path.
- The daemon runs in the systemd slice `yak.slice` as the unit `yak-daemon.<project>.<isolation dir>.<id>`.
- The daemon's process title is `yakd[<project>]`.
- `yak killall` and `yak clean --stale` look for processes named `yak` and `yak-daemon`.
- The client-only build looks for the daemon binary `yak-daemon` next to the client.
- Shell completions register for `yak` and no longer for a `buck` command.
- Remote Execution requests name the tool `yak`.
- The release assets are named `yak-<target triple>`.
- Wheels that `python_wheel` builds name `yak` as their generator in the `WHEEL` file.

""",
)

if failures:
    print("\n".join(failures))
    sys.exit(1)
print("manual edits applied")
