/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

package dev.yak.jvm.cd.serialization.java;

import com.facebook.infer.annotation.Nullsafe;
import com.google.common.collect.ImmutableList;
import dev.yak.core.filesystems.RelPath;
import dev.yak.jvm.cd.serialization.RelPathSerializer;
import dev.yak.jvm.java.ResolvedJavacOptions;
import java.util.Optional;

/** {@link ResolvedJavacOptions} to protobuf serializer */
@Nullsafe(Nullsafe.Mode.LOCAL)
public class ResolvedJavacOptionsSerializer {

  private ResolvedJavacOptionsSerializer() {}

  /**
   * Deserializes javacd model's {@link dev.yak.cd.model.java.ResolvedJavacOptions} into {@link
   * ResolvedJavacOptions}.
   */
  public static ResolvedJavacOptions deserialize(
      dev.yak.cd.model.java.ResolvedJavacOptions options) {
    var bootclasspathListList = options.getBootclasspathListList();
    ImmutableList<RelPath> bootclasspathList =
        bootclasspathListList.stream()
            .map(RelPathSerializer::deserialize)
            .collect(ImmutableList.toImmutableList());

    return new ResolvedJavacOptions(
        bootclasspathList,
        JavacLanguageLevelOptionsSerializer.deserialize(options.getLanguageLevelOptions()),
        true,
        options.getVerbose(),
        JavacPluginParamsSerializer.deserialize(options.getJavaAnnotationProcessorParams()),
        JavacPluginParamsSerializer.deserialize(options.getStandardJavacPluginParams()),
        ImmutableList.copyOf(options.getExtraArgumentsList()),
        options.getSystemImage().isEmpty() ? null : options.getSystemImage());
  }

  private static Optional<String> toOptionalString(String value) {
    return value.isEmpty() ? Optional.empty() : Optional.of(value);
  }
}
