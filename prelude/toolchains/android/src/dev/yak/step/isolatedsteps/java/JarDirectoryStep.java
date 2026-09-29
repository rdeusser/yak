/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

package dev.yak.step.isolatedsteps.java;

import com.google.common.base.MoreObjects;
import com.google.common.collect.ImmutableSet;
import dev.yak.core.build.execution.context.IsolatedExecutionContext;
import dev.yak.core.filesystems.AbsPath;
import dev.yak.core.filesystems.RelPath;
import dev.yak.jvm.java.JarParameters;
import dev.yak.step.StepExecutionResult;
import dev.yak.step.StepExecutionResults;
import dev.yak.step.isolatedsteps.IsolatedStep;
import dev.yak.util.zip.JarBuilder;
import java.io.IOException;
import java.util.Optional;
import java.util.stream.Collectors;

/** Creates a JAR file from a collection of directories/ZIP/JAR files. */
public class JarDirectoryStep implements IsolatedStep {

  private final JarParameters parameters;

  private final ImmutableSet<AbsPath> extraEntriesToJar;

  /**
   * Creates a JAR from the specified entries (most often, classpath entries).
   *
   * <p>If an entry is a directory, then its files are traversed and added to the generated JAR.
   *
   * <p>If an entry is a file, then it is assumed to be a ZIP/JAR file, and its entries will be read
   * and copied to the generated JAR. @Param parameters the parameters that describe how to create
   * the jar.
   */
  public JarDirectoryStep(JarParameters parameters, ImmutableSet<AbsPath> extraEntriesToJar) {
    this.parameters = parameters;
    this.extraEntriesToJar = extraEntriesToJar;
  }

  public JarDirectoryStep(JarParameters parameters) {
    this(parameters, ImmutableSet.of());
  }

  @Override
  public String getShortName() {
    return "jar";
  }

  @Override
  public String getIsolatedStepDescription(IsolatedExecutionContext context) {
    Optional<RelPath> manifestFile = parameters.getManifestFile();
    return String.join(
        " ",
        "jar",
        manifestFile.map(ignore -> "cfm").orElse("cf"),
        parameters.getJarPath().toString(),
        manifestFile.map(RelPath::toString).orElse(""),
        parameters.getEntriesToJar().stream()
            .map(RelPath::toString)
            .collect(Collectors.joining(" ")));
  }

  @Override
  public StepExecutionResult executeIsolatedStep(IsolatedExecutionContext context)
      throws IOException {
    AbsPath root = context.getRuleCellRoot();
    new JarBuilder()
        .setEntriesToJar(extraEntriesToJar.stream())
        .setEntriesToJar(parameters.getEntriesToJar().stream().map(root::resolve))
        .setOverrideEntriesToJar(parameters.getOverrideEntriesToJar().stream().map(root::resolve))
        .setMainClass(parameters.getMainClass().orElse(null))
        .setManifestFile(
            parameters.getManifestFile().map(root::resolve).map(AbsPath::getPath).orElse(null))
        .setShouldMergeManifests(parameters.getMergeManifests())
        .setShouldHashEntries(parameters.getHashEntries())
        .setRemoveEntryPredicate(parameters.getRemoveEntryPredicate())
        .createJarFile(root.resolve(parameters.getJarPath()).getPath());

    return StepExecutionResults.SUCCESS;
  }

  @Override
  public String toString() {
    return MoreObjects.toStringHelper(this).add("parameters", parameters).toString();
  }
}
