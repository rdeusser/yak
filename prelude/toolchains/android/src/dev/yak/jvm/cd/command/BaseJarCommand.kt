/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

package dev.yak.jvm.cd.command

import com.google.common.collect.ImmutableList
import com.google.common.collect.ImmutableMap
import com.google.common.collect.ImmutableSortedSet
import dev.yak.cd.model.java.AbiGenerationMode
import dev.yak.cd.model.java.BaseJarCommand as ProtoBaseJarCommand
import dev.yak.core.filesystems.AbsPath
import dev.yak.core.filesystems.RelPath
import dev.yak.jvm.cd.serialization.AbsPathSerializer
import dev.yak.jvm.cd.serialization.RelPathSerializer
import dev.yak.jvm.cd.serialization.java.BuildTargetValueSerializer
import dev.yak.jvm.cd.serialization.java.CompilerOutputPathsValueSerializer
import dev.yak.jvm.cd.serialization.java.JarParametersSerializer
import dev.yak.jvm.cd.serialization.java.ResolvedJavacOptionsSerializer
import dev.yak.jvm.core.BuildTargetValue
import dev.yak.jvm.java.CompilerOutputPathsValue
import dev.yak.jvm.java.JarParameters
import dev.yak.jvm.java.JdkProvidedInMemoryJavac
import dev.yak.jvm.java.ResolvedJavac
import dev.yak.jvm.java.ResolvedJavacOptions
import java.util.Optional

class BaseJarCommand(
    val abiCompatibilityMode: AbiGenerationMode,
    val abiGenerationMode: AbiGenerationMode,
    val isRequiredForSourceOnlyAbi: Boolean,
    val trackClassUsage: Boolean,
    val compilerOutputPathsValue: CompilerOutputPathsValue,
    val compileTimeClasspathPaths: ImmutableList<RelPath>,
    val compileTimeClasspathSnapshotPathsMap: ImmutableList<RelPath>,
    val javaSrcs: ImmutableSortedSet<RelPath>,
    val resourcesMap: ImmutableMap<RelPath, RelPath>,
    val jarParameters: JarParameters?,
    val buildCellRootPath: AbsPath,
    val resolvedJavac: ResolvedJavac,
    val resolvedJavacOptions: ResolvedJavacOptions,
    val buildTargetValue: BuildTargetValue,
    val buckOut: RelPath,
    val annotationPath: RelPath?,
) {

  companion object {
    fun fromProto(model: ProtoBaseJarCommand, scratchDir: Optional<RelPath>): BaseJarCommand {
      return BaseJarCommand(
          AbiGenerationMode.CLASS,
          model.abiGenerationMode,
          model.trackClassUsage,
          model.trackClassUsage,
          CompilerOutputPathsValueSerializer.deserialize(model.outputPathsValue, scratchDir),
          RelPathSerializer.toListOfRelPath(model.compileTimeClasspathPathsList),
          RelPathSerializer.toListOfRelPath(model.compileTimeClasspathSnapshotPathsList),
          RelPathSerializer.toSortedSetOfRelPath(model.getJavaSrcsList()),
          RelPathSerializer.toResourceMap(model.resourcesMapList),
          if (model.hasJarParameters()) JarParametersSerializer.deserialize(model.jarParameters)
          else null,
          AbsPathSerializer.deserialize(""),
          JdkProvidedInMemoryJavac.createJsr199Javac(),
          ResolvedJavacOptionsSerializer.deserialize(model.resolvedJavacOptions),
          BuildTargetValueSerializer.deserialize(model.buildTargetValue),
          RelPath.get("yak-out/v2"),
          RelPathSerializer.deserialize(model.annotationsPath),
      )
    }
  }
}
