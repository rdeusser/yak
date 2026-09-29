/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

package dev.yak.jvm.kotlin;

import com.google.common.collect.ImmutableList;
import com.google.common.collect.ImmutableSortedSet;
import dev.yak.core.filesystems.AbsPath;
import dev.yak.core.filesystems.RelPath;
import dev.yak.jvm.cd.command.kotlin.LanguageVersion;
import dev.yak.jvm.core.BuildTargetValue;
import dev.yak.jvm.java.CompilerOutputPaths;
import dev.yak.jvm.kotlin.kotlinc.Kotlinc;
import dev.yak.jvm.kotlin.kotlinc.incremental.KotlincMode;
import java.nio.file.Path;
import java.util.Optional;

public class KaptStep extends KotlincStep {

  private static final String VERBOSE = "-verbose";

  KaptStep(
      BuildTargetValue invokingRule,
      Path outputDirectory,
      ImmutableSortedSet<RelPath> sourceFilePaths,
      Path pathToSrcsList,
      ImmutableList<AbsPath> combinedClassPathEntries,
      ImmutableList<AbsPath> kotlinHomeLibraries,
      RelPath reportDirPath,
      Kotlinc kotlinc,
      ImmutableList<String> extraArguments,
      CompilerOutputPaths outputPaths,
      RelPath configuredBuckOut,
      LanguageVersion languageVersion) {
    super(
        invokingRule,
        outputDirectory,
        sourceFilePaths,
        pathToSrcsList,
        combinedClassPathEntries,
        kotlinHomeLibraries,
        reportDirPath,
        kotlinc,
        extraArguments,
        ImmutableList.of(VERBOSE),
        outputPaths,
        false,
        configuredBuckOut,
        Optional.empty(),
        KotlincMode.NonIncremental.INSTANCE,
        languageVersion);
  }

  @Override
  public String getShortName() {
    return "kapt";
  }
}
