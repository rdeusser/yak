/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::io;
use std::path::Path;

use crate::runtime::ZshRuntime;
use crate::verify::extract_from_outputs;

pub(crate) fn run_zsh(
    completion_name: &str,
    script: &str,
    input: &str,
    tempdir: &Path,
) -> io::Result<Vec<String>> {
    let home = tempdir;

    // Copy and paste of `ZshRuntime::new` which works around a zsh bug in which completions are not
    // autoloaded completely
    let config_path = home.join(".zshenv");
    let config = format!(
        "\
fpath=($ZDOTDIR/zsh $fpath)
autoload -U +X compinit && compinit -u # bypass compaudit security checking
precmd_functions=\"\"  # avoid the prompt being overwritten
PS1='%% '
PROMPT='%% '
_{completion_name} >/dev/null 2>/dev/null ; # Force the completion to be loaded
"
    );
    std::fs::write(config_path, config)?;

    let mut r = ZshRuntime::with_home(home.to_owned())?;
    r.register(completion_name, script)?;

    extract_from_outputs(
        input,
        std::iter::empty()
            .chain(std::iter::once_with(|| r.complete(&format!("{input}\t"))))
            .chain(std::iter::once_with(|| r.complete(&format!("{input}\t\t")))),
    )
}
