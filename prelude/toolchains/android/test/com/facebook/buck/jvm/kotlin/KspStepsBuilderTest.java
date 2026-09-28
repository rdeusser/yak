/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

package com.facebook.buck.jvm.kotlin;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertTrue;
import static org.mockito.Mockito.mock;

import com.facebook.buck.core.filesystems.AbsPath;
import com.facebook.buck.core.filesystems.RelPath;
import com.facebook.buck.step.isolatedsteps.IsolatedStep;
import com.facebook.buck.step.isolatedsteps.common.CopyIsolatedStep;
import com.facebook.buck.step.isolatedsteps.common.ZipIsolatedStep;
import com.google.common.collect.ImmutableList;
import org.junit.Test;

public class KspStepsBuilderTest {

  @Test
  public void stagingSteps_copyOutputsBeforeZip() {
    ImmutableList<IsolatedStep> steps =
        KspStepsBuilder.createKspOutputStagingSteps(
            mock(AbsPath.class),
            RelPath.get("kotlin"),
            RelPath.get("java"),
            RelPath.get("classes"),
            RelPath.get("staged"),
            RelPath.get("generated.src.zip"),
            RelPath.get("annotation-output"));

    assertEquals(5, steps.size());
    assertTrue(steps.get(0) instanceof CopyIsolatedStep);
    assertTrue(steps.get(1) instanceof CopyIsolatedStep);
    assertTrue(steps.get(2) instanceof CopyIsolatedStep);
    assertTrue(steps.get(3) instanceof ZipIsolatedStep);
    assertTrue(steps.get(4) instanceof CopyIsolatedStep);
  }
}
