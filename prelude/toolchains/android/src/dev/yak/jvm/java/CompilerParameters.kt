/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

package dev.yak.jvm.java

import com.google.common.collect.ImmutableList
import com.google.common.collect.ImmutableSortedSet
import dev.yak.cd.model.java.AbiGenerationMode
import dev.yak.core.filesystems.RelPath
import dev.yak.jvm.java.abi.source.api.SourceOnlyAbiRuleInfoFactory

data class CompilerParameters(
    val sourceFilePaths: ImmutableSortedSet<RelPath>,
    val classpathEntries: ImmutableList<RelPath>,
    val classpathSnapshots: ImmutableList<RelPath>,
    val outputPaths: CompilerOutputPaths,
    val abiGenerationMode: AbiGenerationMode,
    val abiCompatibilityMode: AbiGenerationMode,
    val shouldTrackClassUsage: Boolean,
    val sourceOnlyAbiRuleInfoFactory: SourceOnlyAbiRuleInfoFactory?,
)
