/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Protobufs for interacting with yak's DownwardApi over gRPC.

// We put this in a module for easier naming in convert.
mod proto {
    tonic::include_proto!("buck.downward_api");
}

pub use proto::*;

mod convert;
