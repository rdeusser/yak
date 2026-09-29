#!/usr/bin/env python3
"""Hand edits of milestone 5 of the rename, which rename_crates.py leaves.

Run from the repository root after `rename_crates.py --apply`. Each edit names
its file, the exact text it replaces, and how many times that text occurs, so
a run on another tree fails instead of editing blindly.
"""

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


# The package, library, and binary of the crate `buck2` take the name `yak`.
edit("app/yak/Cargo.toml", 'name = "buck2"\n', 'name = "yak"\n')
edit("app/yak/YAK", '    name = "buck2",\n', '    name = "yak",\n')
edit("app/yak/YAK", '    crate = "buck2",\n', '    crate = "yak",\n')
edit("app/yak/bin/yak.rs", "use buck2::", "use yak::", count=5)
edit(".vscode/launch.json", '"--package=buck2"', '"--package=yak"')

# The attributes of `yak_bundle` name the binaries they bundle.
edit("YAK", '    buck2 = "//:yak",\n', '    yak = "//:yak",\n')
edit(
    "defs.bzl",
    """    buck2_binary = "yak" + binary_extension
    buck2_daemon_binary = "yak-daemon" + binary_extension

    copied_dir = {}
    materialisations = []

    buck2 = ctx.attrs.buck2[DefaultInfo].default_outputs[0]
    copied_dir[buck2_daemon_binary] = buck2
    materialisations.extend(ctx.attrs.buck2[DefaultInfo].other_outputs)

    yak_client = ctx.attrs.yak_client[DefaultInfo].default_outputs[0]
    copied_dir[buck2_binary] = yak_client
""",
    """    yak_binary = "yak" + binary_extension
    yak_daemon_binary = "yak-daemon" + binary_extension

    copied_dir = {}
    materialisations = []

    yak = ctx.attrs.yak[DefaultInfo].default_outputs[0]
    copied_dir[yak_daemon_binary] = yak
    materialisations.extend(ctx.attrs.yak[DefaultInfo].other_outputs)

    yak_client = ctx.attrs.yak_client[DefaultInfo].default_outputs[0]
    copied_dir[yak_binary] = yak_client
""",
)
edit("defs.bzl", '        "buck2": attrs.dep(),\n', '        "yak": attrs.dep(),\n')

# The error derive macro reads `#[yak(...)]` in place of `#[buck2(...)]`.
edit(
    "app/yak_error_derive/src/lib.rs",
    "#[proc_macro_derive(Error, attributes(error, source, buck2))]",
    "#[proc_macro_derive(Error, attributes(error, source, yak))]",
)
edit("app/yak_error_derive/src/attr.rs", 'attr.path().is_ident("buck2")', 'attr.path().is_ident("yak")')

# Comments that described Meta's layout, where the crates lived in `buck2/app/`.
edit(
    "app/yak_error/src/source_location.rs",
    """        // `yak_error` should only be used within `buck2/app`, giving us a nice way to make sure we
        // strip any leading parts of the path we don't want.
        // Splitting on app/ instead of buck2/app/ to avoid breaking OSS tests where root path is not yak.
""",
    """        // `yak_error` should only be used within `app/`, giving us a nice way to make sure we
        // strip any leading parts of the path we don't want.
""",
)
edit(
    "app/yak_client_ctx/src/common.rs",
    "    ///    analysis/cell//buck2/app/yak_action_impl:yak_action_impl (cfg:linux-x86_64#27ac5723e0c99706)\n",
    "    ///    analysis/cell//app/yak_action_impl:yak_action_impl (cfg:linux-x86_64#27ac5723e0c99706)\n",
)

# Prose that names the crates by a pattern.
edit(
    "ARCHITECTURE.md",
    "- `app/buck2_cmd_*_client` crates implement",
    "- `app/yak_cmd_*_client` crates implement",
)
edit(
    "ARCHITECTURE.md",
    "and the `app/buck2_cmd_*_server` crates implement them",
    "and the `app/yak_cmd_*_server` crates implement them",
)

edit(
    "ARCHITECTURE.md",
    "- The rename to yak continues with the `buck2*` crates.",
    "- The rename to yak continues with the values that still name Buck, such as the"
    " `GOPACKAGESDRIVER_BUCK_OPTIONS` variable and the `buck-headers` directories of the C++ rules.",
)
edit(
    "AGENTS.md",
    "tracks the remaining work, such as the `buck2*` crates.",
    "tracks the remaining work, such as the environment variables and directories that still name Buck.",
)

if failures:
    print("\n".join(failures))
    sys.exit(1)
print("hand edits applied")
