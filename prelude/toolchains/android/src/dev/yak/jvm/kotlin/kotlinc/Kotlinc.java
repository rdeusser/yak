/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

package dev.yak.jvm.kotlin.kotlinc;

import com.google.common.collect.ImmutableList;
import com.google.common.collect.ImmutableSet;
import com.google.common.collect.ImmutableSortedSet;
import dev.yak.core.build.execution.context.IsolatedExecutionContext;
import dev.yak.core.filesystems.AbsPath;
import dev.yak.core.filesystems.RelPath;
import dev.yak.jvm.core.BuildTargetValue;
import dev.yak.jvm.kotlin.kotlinc.incremental.KotlincMode;
import dev.yak.jvm.kotlin.util.KotlinUnarchiverKt;
import java.io.IOException;
import java.nio.file.Path;
import java.util.Optional;

/** Interface for a kotlin compiler. */
public interface Kotlinc {

  int buildWithClasspath(
      IsolatedExecutionContext context,
      BuildTargetValue invokingRule,
      ImmutableList<String> options,
      ImmutableList<AbsPath> kotlinHomeLibraries,
      ImmutableSortedSet<RelPath> kotlinSourceFilePaths,
      Path pathToSrcsList,
      Optional<Path> workingDirectory,
      AbsPath ruleCellRoot,
      KotlincMode mode)
      throws InterruptedException;

  String getDescription(
      ImmutableList<String> options,
      ImmutableSortedSet<RelPath> kotlinSourceFilePaths,
      Path pathToSrcsList);

  String getShortName();

  default ImmutableList<Path> getExpandedSourcePaths(
      AbsPath ruleCellRoot,
      ImmutableSet<RelPath> kotlinSourceFilePaths,
      Optional<Path> workingDirectory)
      throws IOException {

    return KotlinUnarchiverKt.getExpandedSourcePaths(
        ruleCellRoot, kotlinSourceFilePaths, workingDirectory);
  }
}
