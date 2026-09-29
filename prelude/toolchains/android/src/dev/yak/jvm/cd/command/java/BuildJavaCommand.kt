/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

package dev.yak.jvm.cd.command.java

import dev.yak.cd.model.java.BuildCommand as ProtoBuildCommand
import dev.yak.core.filesystems.RelPath
import dev.yak.jvm.cd.command.BaseJarCommand
import dev.yak.jvm.cd.command.BuildMode
import java.util.Optional

class BuildJavaCommand(
    val baseJarCommand: BaseJarCommand,
    val buildMode: BuildMode,
) {

  companion object {
    fun fromProto(model: ProtoBuildCommand, buckScratchPath: Optional<RelPath>): BuildJavaCommand {
      val baseJarCommand: BaseJarCommand =
          BaseJarCommand.fromProto(model.baseJarCommand, buckScratchPath)
      val buildMode: BuildMode = BuildMode.fromProto(model.buildMode)
      return BuildJavaCommand(baseJarCommand, buildMode)
    }
  }
}
