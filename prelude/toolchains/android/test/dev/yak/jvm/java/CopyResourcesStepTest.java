/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

package dev.yak.jvm.java;

import static org.junit.Assert.assertEquals;

import com.google.common.collect.ImmutableList;
import com.google.common.collect.ImmutableMap;
import dev.yak.core.filesystems.RelPath;
import dev.yak.step.isolatedsteps.IsolatedStep;
import dev.yak.step.isolatedsteps.common.MkdirIsolatedStep;
import dev.yak.step.isolatedsteps.common.SymlinkIsolatedStep;
import java.util.List;
import org.junit.Test;

public class CopyResourcesStepTest {

  @Test
  public void testAddResourceCommandsWithBuildFileParentOfSrcDirectory() {
    // Files:
    // android/java/BUCK
    // android/java/src/com/example/base/data.json
    // android/java/src/com/example/common/util/data.json

    RelPath configuredBuckOut = RelPath.get("yak-out/v2");
    RelPath target =
        configuredBuckOut.resolveRel(
            "android/java/lib__resources__classes/com/example/common/util/data.json");
    RelPath target1 =
        configuredBuckOut.resolveRel(
            "android/java/lib__resources__classes/com/example/base/data.json");

    ImmutableMap<RelPath, RelPath> resourceMap =
        ImmutableMap.of(
            RelPath.get("android/java/src/com/example/base/data.json"),
            target1,
            RelPath.get("android/java/src/com/example/common/util/data.json"),
            target);
    ImmutableList<IsolatedStep> steps = CopyResourcesStep.of(resourceMap);

    List<IsolatedStep> expected =
        ImmutableList.of(
            new MkdirIsolatedStep(target1.getParent()),
            new SymlinkIsolatedStep(
                RelPath.get("android/java/src/com/example/base/data.json"), target1),
            new MkdirIsolatedStep(target.getParent()),
            new SymlinkIsolatedStep(
                RelPath.get("android/java/src/com/example/common/util/data.json"), target));
    assertEquals(expected, steps);
  }

  @Test
  public void testAddResourceCommandsWithBuildFileParentOfJavaPackage() {
    // Files:
    // android/java/src/BUCK
    // android/java/src/com/example/base/data.json
    // android/java/src/com/example/common/util/data.json

    RelPath configuredBuckOut = RelPath.get("yak-out/v2");
    RelPath target =
        configuredBuckOut.resolveRel(
            "android/java/src/lib__resources__classes/com/example/common/util/data.json");
    RelPath target1 =
        configuredBuckOut.resolveRel(
            "android/java/src/lib__resources__classes/com/example/base/data.json");

    ImmutableMap<RelPath, RelPath> resourceMap =
        ImmutableMap.of(
            RelPath.get("android/java/src/com/example/base/data.json"),
            target1,
            RelPath.get("android/java/src/com/example/common/util/data.json"),
            target);
    ImmutableList<IsolatedStep> steps = CopyResourcesStep.of(resourceMap);

    List<IsolatedStep> expected =
        ImmutableList.of(
            new MkdirIsolatedStep(target1.getParent()),
            new SymlinkIsolatedStep(
                RelPath.get("android/java/src/com/example/base/data.json"), target1),
            new MkdirIsolatedStep(target.getParent()),
            new SymlinkIsolatedStep(
                RelPath.get("android/java/src/com/example/common/util/data.json"), target));
    assertEquals(expected, steps);
  }

  @Test
  public void testAddResourceCommandsWithBuildFileInJavaPackage() {
    // Files:
    // android/java/src/com/example/BUCK
    // android/java/src/com/example/base/data.json
    // android/java/src/com/example/common/util/data.json

    RelPath configuredBuckOut = RelPath.get("yak-out/v2");
    RelPath target =
        configuredBuckOut.resolveRel(
            "android/java/src/com/example/lib__resources__classes/"
                + "com/example/common/util/data.json");
    RelPath target1 =
        configuredBuckOut.resolveRel(
            "android/java/src/com/example/lib__resources__classes/" + "com/example/base/data.json");

    ImmutableMap<RelPath, RelPath> resourceMap =
        ImmutableMap.of(
            RelPath.get("android/java/src/com/example/base/data.json"),
            target1,
            RelPath.get("android/java/src/com/example/common/util/data.json"),
            target);
    ImmutableList<IsolatedStep> steps = CopyResourcesStep.of(resourceMap);

    List<IsolatedStep> expected =
        ImmutableList.of(
            new MkdirIsolatedStep(target1.getParent()),
            new SymlinkIsolatedStep(
                RelPath.get("android/java/src/com/example/base/data.json"), target1),
            new MkdirIsolatedStep(target.getParent()),
            new SymlinkIsolatedStep(
                RelPath.get("android/java/src/com/example/common/util/data.json"), target));
    assertEquals(expected, steps);
  }
}
