/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

package dev.yak.step.isolatedsteps.common

import dev.yak.core.build.execution.context.IsolatedExecutionContext
import dev.yak.core.filesystems.RelPath
import dev.yak.io.filesystem.impl.ProjectFilesystemUtils
import dev.yak.step.StepExecutionResult
import dev.yak.step.StepExecutionResults
import dev.yak.step.isolatedsteps.IsolatedStep
import java.io.IOException

/** Creates a symlink from a desired path to an existing path. */
data class SymlinkIsolatedStep(val existingPath: RelPath, val desiredPath: RelPath) : IsolatedStep {
  override fun getShortName(): String {
    return "symlink_file"
  }

  @Throws(IOException::class)
  override fun executeIsolatedStep(context: IsolatedExecutionContext): StepExecutionResult {
    val ruleCellRoot = context.ruleCellRoot

    val existingAbsPath =
        ProjectFilesystemUtils.getAbsPathForRelativePath(ruleCellRoot, existingPath)
    val desiredAbsPath = ProjectFilesystemUtils.getAbsPathForRelativePath(ruleCellRoot, desiredPath)

    ProjectFilesystemUtils.createSymLink(
        ruleCellRoot,
        desiredAbsPath.path,
        existingAbsPath.path, /* force */
        true,
    )

    return StepExecutionResults.SUCCESS
  }

  override fun getIsolatedStepDescription(context: IsolatedExecutionContext): String {
    val ruleCellRoot = context.ruleCellRoot
    return java.lang.String.join(
        " ",
        "ln",
        "-f",
        "-s",
        ProjectFilesystemUtils.getAbsPathForRelativePath(ruleCellRoot, existingPath).toString(),
        ProjectFilesystemUtils.getAbsPathForRelativePath(ruleCellRoot, desiredPath).toString(),
    )
  }
}
