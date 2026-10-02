/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Verifies the shell completions of a command by completing input in bash, fish, or zsh through
//! a pseudo-terminal, which exists only on Unix.

#[cfg(unix)]
mod bash;
#[cfg(unix)]
mod fish;
#[cfg(unix)]
mod runtime;
#[cfg(unix)]
mod verify;
#[cfg(unix)]
mod zsh;

#[cfg(unix)]
fn main() -> std::io::Result<()> {
    verify::main()
}

#[cfg(not(unix))]
fn main() {
    eprintln!("completion_verify runs only on Unix");
    std::process::exit(1);
}
