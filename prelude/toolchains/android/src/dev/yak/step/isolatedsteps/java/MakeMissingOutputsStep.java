/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

package dev.yak.step.isolatedsteps.java;

import dev.yak.core.build.execution.context.IsolatedExecutionContext;
import dev.yak.core.filesystems.AbsPath;
import dev.yak.core.filesystems.RelPath;
import dev.yak.io.filesystem.impl.ProjectFilesystemUtils;
import dev.yak.step.StepExecutionResult;
import dev.yak.step.StepExecutionResults;
import dev.yak.step.isolatedsteps.common.AbstractIsolatedExecutionStep;
import java.io.IOException;

/** Make Missing Output directories step. */
public class MakeMissingOutputsStep extends AbstractIsolatedExecutionStep {

  private final RelPath annotationsPath;

  public MakeMissingOutputsStep(RelPath annotationsPath) {
    super("make_missing_outputs");
    this.annotationsPath = annotationsPath;
  }

  @Override
  public StepExecutionResult executeIsolatedStep(IsolatedExecutionContext context)
      throws IOException {
    AbsPath root = context.getRuleCellRoot();
    if (!ProjectFilesystemUtils.exists(root, annotationsPath.getPath())) {
      ProjectFilesystemUtils.mkdirs(root, annotationsPath.getPath());
    }
    return StepExecutionResults.SUCCESS;
  }
}
