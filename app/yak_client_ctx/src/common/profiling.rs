/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use dupe::Dupe;

#[derive(
    clap::ValueEnum,
    Dupe,
    Clone,
    Copy,
    Debug,
    serde::Serialize,
    serde::Deserialize
)]
pub enum YakProfileMode {
    TimeFlame,
    HeapAllocated,
    HeapRetained,
    HeapFlameAllocated,
    HeapFlameRetained,
    HeapSummaryAllocated,
    HeapSummaryRetained,
    Statement,
    Bytecode,
    BytecodePairs,
    Typecheck,
    Coverage,
    None,
}

impl YakProfileMode {
    pub fn to_proto(&self) -> yak_cli_proto::ProfileMode {
        match self {
            YakProfileMode::TimeFlame => yak_cli_proto::ProfileMode::TimeFlame,
            YakProfileMode::HeapAllocated => yak_cli_proto::ProfileMode::HeapAllocated,
            YakProfileMode::HeapRetained => yak_cli_proto::ProfileMode::HeapRetained,
            YakProfileMode::HeapFlameAllocated => yak_cli_proto::ProfileMode::HeapFlameAllocated,
            YakProfileMode::HeapFlameRetained => yak_cli_proto::ProfileMode::HeapFlameRetained,
            YakProfileMode::HeapSummaryAllocated => {
                yak_cli_proto::ProfileMode::HeapSummaryAllocated
            }
            YakProfileMode::HeapSummaryRetained => yak_cli_proto::ProfileMode::HeapSummaryRetained,
            YakProfileMode::Statement => yak_cli_proto::ProfileMode::Statement,
            YakProfileMode::Bytecode => yak_cli_proto::ProfileMode::Bytecode,
            YakProfileMode::BytecodePairs => yak_cli_proto::ProfileMode::BytecodePairs,
            YakProfileMode::Typecheck => yak_cli_proto::ProfileMode::Typecheck,
            YakProfileMode::Coverage => yak_cli_proto::ProfileMode::Coverage,
            YakProfileMode::None => yak_cli_proto::ProfileMode::None,
        }
    }
}
