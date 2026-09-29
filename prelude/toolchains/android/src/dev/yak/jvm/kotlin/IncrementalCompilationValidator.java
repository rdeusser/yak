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
import dev.yak.core.filesystems.AbsPath;
import dev.yak.core.filesystems.RelPath;
import dev.yak.jvm.java.ActionMetadata;
import dev.yak.jvm.kotlin.kotlinc.incremental.RebuildReason;
import javax.annotation.Nullable;

public class IncrementalCompilationValidator {

  public IncrementalCompilationValidator() {}

  public @Nullable RebuildReason validate(
      ImmutableList<RelPath> kotlinCompilerPlugins,
      ActionMetadata actionMetadata,
      @Nullable AbsPath depFile,
      @Nullable AbsPath usedJars,
      @Nullable AbsPath jvmAbiGenWorkingDir) {
    if (actionMetadata.getPreviousIncrementalConfigDigest() == null) {
      return RebuildReason.NO_LAST_BUILD_CONFIGURATION;
    }

    if (!actionMetadata
        .getPreviousIncrementalConfigDigest()
        .equals(actionMetadata.getCurrentIncrementalConfigDigest())) {
      return RebuildReason.BUILD_CONFIGURATION_CHANGED;
    }

    if (depFile != null && !depFile.toFile().exists()) {
      return RebuildReason.NO_LAST_DEP_FILE;
    }

    if (usedJars != null && !usedJars.toFile().exists()) {
      return RebuildReason.NO_LAST_USED_JARS;
    }

    if (jvmAbiGenWorkingDir != null && !jvmAbiGenWorkingDir.toFile().exists()) {
      return RebuildReason.NO_JVM_ABI_WORKING_DIR;
    }

    JarsActionMetadata jarsActionMetadata = new JarsActionMetadata(actionMetadata);
    if (kotlinCompilerPlugins.stream().anyMatch(jarsActionMetadata::hasChanged)) {
      return RebuildReason.KOTLIN_COMPILER_PLUGIN_CHANGED;
    }

    return null;
  }
}
