/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

package dev.yak.jvm.cd;

import com.facebook.infer.annotation.Nullsafe;
import com.google.common.collect.ImmutableList;
import com.google.common.collect.ImmutableMap;
import com.google.common.collect.ImmutableSortedSet;
import dev.yak.cd.model.java.AbiGenerationMode;
import dev.yak.core.filesystems.AbsPath;
import dev.yak.core.filesystems.RelPath;
import dev.yak.jvm.core.BuildTargetValue;
import dev.yak.jvm.java.ActionMetadata;
import dev.yak.jvm.java.CompileToJarStepFactory;
import dev.yak.jvm.java.CompilerOutputPathsValue;
import dev.yak.jvm.java.JarParameters;
import dev.yak.jvm.java.ResolvedJavac;
import org.jetbrains.annotations.Nullable;

/** Builder that creates library jar steps. */
@Nullsafe(Nullsafe.Mode.LOCAL)
public interface LibraryStepsBuilder extends CompileStepsBuilder {

  void addMakeMissingOutputsStep(RelPath annotationsPath);

  void addBuildStepsForLibrary(
      AbiGenerationMode abiCompatibilityMode,
      AbiGenerationMode abiGenerationMode,
      boolean isRequiredForSourceOnlyAbi,
      boolean trackClassUsage,
      RelPath buckOut,
      BuildTargetValue buildTargetValue,
      CompilerOutputPathsValue compilerOutputPathsValue,
      ImmutableList<RelPath> compileTimeClasspathPaths,
      ImmutableList<RelPath> compileTimeClasspathSnapshotPaths,
      ImmutableSortedSet<RelPath> javaSrcs,
      ImmutableMap<RelPath, RelPath> resourcesMap,
      @Nullable JarParameters libraryJarParameters,
      AbsPath buildCellRootPath,
      ResolvedJavac resolvedJavac,
      @Nullable ActionMetadata actionMetadata,
      CompileToJarStepFactory.ExtraParams extraParams);
}
