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

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertThrows;
import static org.junit.Assert.assertTrue;

import com.google.common.collect.ImmutableList;
import com.google.common.collect.ImmutableSortedSet;
import dev.yak.cd.model.java.BuildTargetValue.Type;
import dev.yak.core.build.execution.context.IsolatedExecutionContext;
import dev.yak.core.filesystems.AbsPath;
import dev.yak.core.filesystems.RelPath;
import dev.yak.jvm.cd.command.kotlin.LanguageVersion;
import dev.yak.jvm.core.BuildTargetValue;
import dev.yak.jvm.java.CompilerOutputPaths;
import dev.yak.jvm.kotlin.kotlinc.Kotlinc;
import dev.yak.jvm.kotlin.kotlinc.incremental.KotlincMode;
import dev.yak.step.StepExecutionResult;
import dev.yak.step.TestExecutionContext;
import dev.yak.testutil.TemporaryPaths;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.Optional;
import org.junit.Rule;
import org.junit.Test;

public class KotlincStepTest {
  @Rule public TemporaryPaths tmp = new TemporaryPaths();

  @Test
  public void sourceOnlyAbiIsRejected() {
    KotlincStep step =
        createStep(Type.SOURCE_ONLY_ABI, new FakeKotlinc(0, ""), false, Optional.empty());
    IsolatedExecutionContext context = TestExecutionContext.newInstance(tmp.getRoot());

    Error error = assertThrows(Error.class, () -> step.getOptions(context, ImmutableList.of()));
    assertTrue(error.getMessage().contains("Source-only ABI"));
  }

  @Test
  public void trackingClassUsageRequiresDepTracker() {
    KotlincStep step = createStep(Type.LIBRARY, new FakeKotlinc(0, ""), true, Optional.empty());
    IsolatedExecutionContext context = TestExecutionContext.newInstance(tmp.getRoot());

    IllegalStateException error =
        assertThrows(
            IllegalStateException.class, () -> step.getOptions(context, ImmutableList.of()));
    assertTrue(error.getMessage().contains("track_class_usage_plugin"));
  }

  @Test
  public void trackingClassUsageAddsDepTrackerPlugin() {
    AbsPath depTracker = tmp.getRoot().resolve("dep-tracker.jar");
    KotlincStep step =
        createStep(Type.LIBRARY, new FakeKotlinc(0, ""), true, Optional.of(depTracker));
    IsolatedExecutionContext context = TestExecutionContext.newInstance(tmp.getRoot());

    ImmutableList<String> options = step.getOptions(context, ImmutableList.of());

    int pluginIndex = options.indexOf("-Xplugin=" + depTracker);
    assertTrue(pluginIndex >= 0);
    assertEquals("-P", options.get(pluginIndex + 1));
    assertTrue(options.get(pluginIndex + 2).startsWith("plugin:buck_deps_tracker:out="));
  }

  @Test
  public void failedCompileReturnsStderr() {
    KotlincStep step =
        createStep(Type.LIBRARY, new FakeKotlinc(1, "kotlinc stderr\n"), false, Optional.empty());
    IsolatedExecutionContext context = TestExecutionContext.newInstance(tmp.getRoot());

    StepExecutionResult result = step.executeIsolatedStep(context);

    assertEquals(1, result.getExitCode());
    assertEquals(Optional.of("kotlinc stderr\n"), result.getStderr());
  }

  private static KotlincStep createStep(
      Type targetType, Kotlinc kotlinc, boolean trackClassUsage, Optional<AbsPath> depTracker) {
    return new KotlincStep(
        new BuildTargetValue(targetType, "//foo:bar"),
        Paths.get("classes"),
        ImmutableSortedSet.of(),
        Paths.get("srcs.txt"),
        ImmutableList.of(),
        ImmutableList.of(),
        RelPath.get("reports"),
        kotlinc,
        ImmutableList.of(),
        ImmutableList.of(),
        new CompilerOutputPaths(
            RelPath.get("classesDir"),
            RelPath.get("outputJarDirPath"),
            Optional.empty(),
            RelPath.get("annotationPath"),
            RelPath.get("pathToSourcesList"),
            RelPath.get("workingDirectory"),
            Optional.empty()),
        trackClassUsage,
        RelPath.get("yak-out/v2"),
        depTracker,
        KotlincMode.NonIncremental.INSTANCE,
        new LanguageVersion("2.1"));
  }

  /** FakeKotlinc writes a fixed message to stderr and returns a fixed exit code. */
  private static class FakeKotlinc implements Kotlinc {
    private final int exitCode;
    private final String stdErr;

    FakeKotlinc(int exitCode, String stdErr) {
      this.exitCode = exitCode;
      this.stdErr = stdErr;
    }

    @Override
    public int buildWithClasspath(
        IsolatedExecutionContext context,
        BuildTargetValue invokingRule,
        ImmutableList<String> options,
        ImmutableList<AbsPath> kotlinHomeLibraries,
        ImmutableSortedSet<RelPath> kotlinSourceFilePaths,
        Path pathToSrcsList,
        Optional<Path> workingDirectory,
        AbsPath ruleCellRoot,
        KotlincMode mode) {
      context.getStdErr().print(stdErr);
      return exitCode;
    }

    @Override
    public String getDescription(
        ImmutableList<String> options,
        ImmutableSortedSet<RelPath> kotlinSourceFilePaths,
        Path pathToSrcsList) {
      return "fakeKotlinc";
    }

    @Override
    public String getShortName() {
      return "fakeKotlinc";
    }
  }
}
