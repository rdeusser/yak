# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.


import os

import pytest
from e2e_util.api.yak import Yak
from e2e_util.yak_workspace import yak_test

# TODO(nga): Local and remote execution of `//:dog_and_bone` must produce identical output.
#   This is a known limitation of at least our RE implementation. It reads through symlinks.


@yak_test(skip_for_os=["windows"])
async def test_symlink_preserves_empty_directory_local(yak: Yak) -> None:
    result = await yak.build("//:dog_and_bone", "--prefer-local", "--no-remote-cache")
    out = result.get_build_report().output_for_target("//:dog_and_bone")
    assert os.path.islink(out)
    assert os.path.isfile(out)


@pytest.mark.remote_execution
@yak_test(skip_for_os=["windows"])
async def test_symlink_preserves_empty_directory_remote(yak: Yak) -> None:
    result = await yak.build("//:dog_and_bone", "--prefer-remote")
    out = result.get_build_report().output_for_target("//:dog_and_bone")
    # This is incorrect, should be a symlink.
    assert not os.path.islink(out)
    assert os.path.isfile(out)
