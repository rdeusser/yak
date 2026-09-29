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
import com.google.common.collect.ImmutableSortedSet;
import dev.yak.cd.model.java.AbiGenerationMode;
import dev.yak.core.filesystems.RelPath;
import dev.yak.jvm.java.CompilerOutputPaths;
import dev.yak.jvm.java.CompilerParameters;
import dev.yak.jvm.java.DefaultSourceOnlyAbiRuleInfoFactory;

/** Common utilities. */
@Nullsafe(Nullsafe.Mode.LOCAL)
public class JavaLibraryRules {

  /** Utility class: do not instantiate. */
  private JavaLibraryRules() {}

  /** Creates {@link CompilerParameters} */
  public static CompilerParameters getCompilerParameters(
      ImmutableList<RelPath> compileTimeClasspathPaths,
      ImmutableList<RelPath> compileTimeClasspathSnapshotPaths,
      ImmutableSortedSet<RelPath> javaSrcs,
      String fullyQualifiedBuildTargetName,
      boolean trackClassUsage,
      AbiGenerationMode abiGenerationMode,
      AbiGenerationMode abiCompatibilityMode,
      boolean isRequiredForSourceOnlyAbi,
      CompilerOutputPaths compilerOutputPaths) {
    return new CompilerParameters(
        javaSrcs,
        compileTimeClasspathPaths,
        compileTimeClasspathSnapshotPaths,
        compilerOutputPaths,
        abiGenerationMode,
        abiCompatibilityMode,
        trackClassUsage,
        new DefaultSourceOnlyAbiRuleInfoFactory(
            fullyQualifiedBuildTargetName, isRequiredForSourceOnlyAbi));
  }
}
