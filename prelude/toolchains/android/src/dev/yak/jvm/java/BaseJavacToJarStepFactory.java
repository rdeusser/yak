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

import com.google.common.collect.ImmutableList;
import dev.yak.core.filesystems.AbsPath;
import dev.yak.core.filesystems.RelPath;
import dev.yak.jvm.core.BuildTargetValue;
import dev.yak.step.isolatedsteps.IsolatedStep;
import dev.yak.step.isolatedsteps.common.MkdirIsolatedStep;
import javax.annotation.Nullable;

/**
 * Java implementation of compile to jar steps factory that doesn't depend on internal build graph
 * datastructures, and only knows how to create compile steps.
 */
public class BaseJavacToJarStepFactory extends BaseCompileToJarStepFactory<JavaExtraParams> {
  public BaseJavacToJarStepFactory() {}

  @Override
  public JavaExtraParams castExtraParams(ExtraParams extraParams) {
    return (JavaExtraParams) extraParams;
  }

  @Override
  public void createCompileStep(
      RelPath buckOut,
      AbsPath buildCellRootPath,
      BuildTargetValue invokingRule,
      CompilerOutputPathsValue compilerOutputPathsValue,
      CompilerParameters parameters,
      ImmutableList.Builder<IsolatedStep> steps,
      ResolvedJavac resolvedJavac,
      @Nullable ActionMetadata actionMetadata,
      JavaExtraParams extraParams,
      JarParameters abiJarParameters,
      boolean mixedCompilation) {

    CompilerOutputPaths outputPath = compilerOutputPathsValue.getByType(invokingRule.getType());
    if (extraParams.getAddAnnotationPath()) {
      addAnnotationGenFolderStep(steps, outputPath.getAnnotationPath());
    }

    ResolvedJavacOptions resolvedJavacOptions = extraParams.getResolvedJavacOptions();
    steps.add(
        new JavacStep(
            resolvedJavac,
            resolvedJavacOptions,
            invokingRule,
            buckOut,
            compilerOutputPathsValue,
            parameters,
            extraParams.getSkipIfNoCompilationUnits(),
            abiJarParameters,
            null,
            mixedCompilation));
  }

  protected void addAnnotationGenFolderStep(
      ImmutableList.Builder<IsolatedStep> steps, RelPath annotationGenFolder) {
    steps.add(new MkdirIsolatedStep(annotationGenFolder));
  }
}
