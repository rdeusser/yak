# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

from e2e_util.api.yak import Yak
from e2e_util.yak_workspace import yak_test


@yak_test()
async def test_read_root_config(yak: Yak) -> None:
    output = await yak.build("//:")
    assert "<<root=regular>>" in output.stderr
    assert "<<root_ignore_default=regular>>" in output.stderr
    assert "<<root_use_default=predict>>" in output.stderr
    assert "<<local=regular>>" in output.stderr

    output = await yak.build("other//:")
    assert "{{root=regular}}" in output.stderr
    assert "{{root_ignore_default=regular}}" in output.stderr
    assert "{{root_use_default=quantity}}" in output.stderr
    assert "{{local=guerrilla}}" in output.stderr
