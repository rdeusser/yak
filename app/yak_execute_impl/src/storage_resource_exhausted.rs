/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use remote_execution::TCode;
use remote_execution::TStatus;
use yak_execute::re::error::RemoteExecutionError;

#[allow(dead_code)]
pub(crate) trait REErrorWithCodeAndMessage {
    fn message(&self) -> &str;
    fn code(&self) -> &TCode;
}

impl REErrorWithCodeAndMessage for RemoteExecutionError {
    fn message(&self) -> &str {
        &self.message
    }

    fn code(&self) -> &TCode {
        &self.code
    }
}

impl REErrorWithCodeAndMessage for TStatus {
    fn message(&self) -> &str {
        &self.message
    }

    fn code(&self) -> &TCode {
        &self.code
    }
}

/// is_storage_resource_exhausted reports whether an RE error means CAS storage is exhausted, which
/// the hybrid executor does not retry locally. The Remote Execution API uses `RESOURCE_EXHAUSTED`
/// for storage and for other exhausted quotas alike, so no error is classified this way.
pub(crate) fn is_storage_resource_exhausted<T: REErrorWithCodeAndMessage>(_err: &T) -> bool {
    false
}
