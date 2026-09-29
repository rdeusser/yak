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
pub enum BuckProfileMode {
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

impl BuckProfileMode {
    pub fn to_proto(&self) -> yak_cli_proto::ProfileMode {
        match self {
            BuckProfileMode::TimeFlame => yak_cli_proto::ProfileMode::TimeFlame,
            BuckProfileMode::HeapAllocated => yak_cli_proto::ProfileMode::HeapAllocated,
            BuckProfileMode::HeapRetained => yak_cli_proto::ProfileMode::HeapRetained,
            BuckProfileMode::HeapFlameAllocated => yak_cli_proto::ProfileMode::HeapFlameAllocated,
            BuckProfileMode::HeapFlameRetained => yak_cli_proto::ProfileMode::HeapFlameRetained,
            BuckProfileMode::HeapSummaryAllocated => {
                yak_cli_proto::ProfileMode::HeapSummaryAllocated
            }
            BuckProfileMode::HeapSummaryRetained => yak_cli_proto::ProfileMode::HeapSummaryRetained,
            BuckProfileMode::Statement => yak_cli_proto::ProfileMode::Statement,
            BuckProfileMode::Bytecode => yak_cli_proto::ProfileMode::Bytecode,
            BuckProfileMode::BytecodePairs => yak_cli_proto::ProfileMode::BytecodePairs,
            BuckProfileMode::Typecheck => yak_cli_proto::ProfileMode::Typecheck,
            BuckProfileMode::Coverage => yak_cli_proto::ProfileMode::Coverage,
            BuckProfileMode::None => yak_cli_proto::ProfileMode::None,
        }
    }
}
