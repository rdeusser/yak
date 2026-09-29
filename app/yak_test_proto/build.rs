/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::env;
use std::io;

fn main() -> io::Result<()> {
    let proto_files = &["test.proto"];

    let yak_proto_srcs = env::var("YAK_PROTO_SRCS");
    let includes = if let Ok(path) = &yak_proto_srcs {
        vec![path.as_str()]
    } else {
        vec![".", "../yak_data", "../yak_host_sharing_proto"]
    };

    let builder = yak_protoc_dev::configure();
    unsafe { builder.setup_protoc() }
        .type_attribute(
            "yak.test.ExecuteResponse2.response",
            "#[allow(clippy::large_enum_variant)]",
        )
        .extern_path(".yak.data", "::yak_data")
        .extern_path(".yak.host_sharing", "::yak_host_sharing_proto")
        .compile(proto_files, &includes)
}
