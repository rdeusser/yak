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
import com.google.common.collect.ImmutableSortedSet;
import dev.yak.core.filesystems.RelPath;
import dev.yak.jvm.cd.serialization.RelPathSerializer;
import dev.yak.jvm.java.JarParameters;
import java.util.Optional;
import java.util.logging.Level;

/** {@link JarParameters} to protobuf serializer */
@Nullsafe(Nullsafe.Mode.LOCAL)
public class JarParametersSerializer {

  private JarParametersSerializer() {}

  /**
   * Deserializes javacd model's {@link dev.yak.cd.model.java.JarParameters} into {@link
   * JarParameters}.
   */
  public static JarParameters deserialize(dev.yak.cd.model.java.JarParameters jarParameters) {
    Optional<String> mainClass =
        Optional.of(jarParameters.getMainClass()).filter(s -> !s.isEmpty());
    Optional<RelPath> manifestFile =
        Optional.of(jarParameters.getManifestFile())
            .filter(s -> !s.isEmpty())
            .map(RelPathSerializer::deserialize);

    return new JarParameters(
        jarParameters.getHashEntries(),
        jarParameters.getMergeManifests(),
        RelPathSerializer.deserialize(jarParameters.getJarPath()),
        RemoveClassesPatternsMatcherSerializer.deserialize(jarParameters.getRemoveEntryPredicate()),
        jarParameters.getEntriesToJarList().stream()
            .map(RelPathSerializer::deserialize)
            .collect(ImmutableSortedSet.toImmutableSortedSet(RelPath.comparator())),
        jarParameters.getOverrideEntriesToJarList().stream()
            .map(RelPathSerializer::deserialize)
            .collect(ImmutableSortedSet.toImmutableSortedSet(RelPath.comparator())),
        mainClass,
        manifestFile,
        Level.FINE);
  }
}
