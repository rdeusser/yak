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

import static com.google.common.collect.Iterables.transform;
import static dev.yak.jvm.java.CompilerOutputPaths.getKotlinTempDepFilePath;

import com.google.common.annotations.VisibleForTesting;
import com.google.common.base.Joiner;
import com.google.common.collect.ImmutableList;
import com.google.common.collect.ImmutableSortedSet;
import dev.yak.core.build.execution.context.IsolatedExecutionContext;
import dev.yak.core.filesystems.AbsPath;
import dev.yak.core.filesystems.RelPath;
import dev.yak.core.util.log.Logger;
import dev.yak.jvm.cd.command.kotlin.LanguageVersion;
import dev.yak.jvm.core.BuildTargetValue;
import dev.yak.jvm.java.CompilerOutputPaths;
import dev.yak.jvm.kotlin.kotlinc.Kotlinc;
import dev.yak.jvm.kotlin.kotlinc.incremental.KotlincMode;
import dev.yak.step.StepExecutionResult;
import dev.yak.step.StepExecutionResults;
import dev.yak.step.isolatedsteps.IsolatedStep;
import dev.yak.util.CapturingPrintStream;
import dev.yak.util.Verbosity;
import java.io.File;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Path;
import java.time.Duration;
import java.time.Instant;
import java.util.Optional;

/** Kotlin compile Step */
public class KotlincStep implements IsolatedStep {
  private static final Logger LOG = Logger.get(KotlincStep.class);

  private static final String CLASSPATH_FLAG = "-classpath";
  private static final String DESTINATION_FLAG = "-d";
  private static final String X_PLUGIN_ARG = "-Xplugin=";
  private static final String PLUGIN = "-P";

  private final Kotlinc kotlinc;
  private final ImmutableList<AbsPath> combinedClassPathEntries;
  private final ImmutableList<AbsPath> kotlinHomeLibraries;
  private final Path outputDirectory;
  private final ImmutableList<String> extraArguments;
  private final ImmutableList<String> verboseModeOnlyExtraArguments;
  private final ImmutableSortedSet<RelPath> sourceFilePaths;
  private final Path pathToSrcsList;
  private final RelPath reportDirPath;
  private final BuildTargetValue invokingRule;
  private final CompilerOutputPaths outputPaths;
  private final boolean trackClassUsage;
  private final RelPath configuredBuckOut;
  private final Optional<AbsPath> depTrackerPath;
  private final KotlincMode kotlincMode;
  private final LanguageVersion languageVersion;

  KotlincStep(
      BuildTargetValue invokingRule,
      Path outputDirectory,
      ImmutableSortedSet<RelPath> sourceFilePaths,
      Path pathToSrcsList,
      ImmutableList<AbsPath> combinedClassPathEntries,
      ImmutableList<AbsPath> kotlinHomeLibraries,
      RelPath reportDirPath,
      Kotlinc kotlinc,
      ImmutableList<String> extraArguments,
      ImmutableList<String> verboseModeOnlyExtraArguments,
      CompilerOutputPaths outputPaths,
      boolean trackClassUsage,
      RelPath configuredBuckOut,
      Optional<AbsPath> depTrackerPath,
      KotlincMode kotlincMode,
      LanguageVersion languageVersion) {
    this.invokingRule = invokingRule;
    this.outputDirectory = outputDirectory;
    this.sourceFilePaths = sourceFilePaths;
    this.pathToSrcsList = pathToSrcsList;
    this.reportDirPath = reportDirPath;
    this.kotlinc = kotlinc;
    this.combinedClassPathEntries = combinedClassPathEntries;
    this.kotlinHomeLibraries = kotlinHomeLibraries;
    this.extraArguments = extraArguments;
    this.verboseModeOnlyExtraArguments = verboseModeOnlyExtraArguments;
    this.outputPaths = outputPaths;
    this.trackClassUsage = trackClassUsage;
    this.configuredBuckOut = configuredBuckOut;
    this.depTrackerPath = depTrackerPath;
    this.kotlincMode = kotlincMode;
    this.languageVersion = languageVersion;
  }

  @Override
  public String getShortName() {
    return getKotlinc().getShortName();
  }

  @Override
  public StepExecutionResult executeIsolatedStep(IsolatedExecutionContext context) {
    ImmutableList<String> compilerOptions = getOptions(context, combinedClassPathEntries);
    Verbosity verbosity =
        context.getVerbosity().isSilent() ? Verbosity.STANDARD_INFORMATION : context.getVerbosity();
    try (CapturingPrintStream stdout = new CapturingPrintStream();
        CapturingPrintStream stderr = new CapturingPrintStream();
        IsolatedExecutionContext firstOrderContext =
            context.createSubContext(stdout, stderr, Optional.of(verbosity))) {

      Instant compilationStart = Instant.now();

      int declaredDepsBuildResult =
          kotlinc.buildWithClasspath(
              firstOrderContext,
              invokingRule,
              compilerOptions,
              kotlinHomeLibraries,
              sourceFilePaths,
              pathToSrcsList,
              Optional.of(outputPaths.getWorkingDirectory().getPath()),
              context.getRuleCellRoot(),
              kotlincMode);

      Instant compilationEnd = Instant.now();
      Duration compilationDuration = Duration.between(compilationStart, compilationEnd);
      LOG.info(
          "KOTLINCD_STEP_DURATION|%s|%s|%d|%d",
          invokingRule.getFullyQualifiedName(),
          this.getClass().getSimpleName(),
          compilationDuration.toMillis(),
          sourceFilePaths.size());

      Optional<String> returnedStderr =
          declaredDepsBuildResult == StepExecutionResults.SUCCESS_EXIT_CODE
              ? Optional.empty()
              : Optional.of(stderr.getContentsAsString(StandardCharsets.UTF_8));

      if (declaredDepsBuildResult == StepExecutionResults.SUCCESS_EXIT_CODE && trackClassUsage) {
        AbsPath ruleCellRoot = context.getRuleCellRoot();
        RelPath outputJarDirPath = outputPaths.getOutputJarDirPath();
        ClassUsageFileWriterFactory.create(kotlincMode)
            .writeFile(
                KotlinClassUsageHelper.getClassUsageData(reportDirPath, ruleCellRoot),
                CompilerOutputPaths.getKotlinDepFilePath(outputJarDirPath),
                ruleCellRoot,
                configuredBuckOut);
      }

      return new StepExecutionResult(declaredDepsBuildResult, returnedStderr);
    } catch (IOException | InterruptedException e) {
      throw new RuntimeException(e);
    }
  }

  @VisibleForTesting
  Kotlinc getKotlinc() {
    return kotlinc;
  }

  @Override
  public String getIsolatedStepDescription(IsolatedExecutionContext context) {
    return getKotlinc()
        .getDescription(
            getOptions(context, getClasspathEntries()), sourceFilePaths, pathToSrcsList);
  }

  /**
   * Returns a list of command-line options to pass to javac. These options reflect the
   * configuration of this javac command.
   *
   * @param context the ExecutionContext with in which javac will run
   * @return list of String command-line options.
   */
  @VisibleForTesting
  ImmutableList<String> getOptions(
      IsolatedExecutionContext context, ImmutableList<AbsPath> buildClasspathEntries) {
    ImmutableList.Builder<String> builder = ImmutableList.builder();

    AbsPath ruleCellRoot = context.getRuleCellRoot();

    if (outputDirectory != null) {
      builder.add(DESTINATION_FLAG, ruleCellRoot.resolve(outputDirectory).toString());
    }

    if (invokingRule.isSourceOnlyAbi()) {
      throw new Error("Source-only ABI flavor is not supported for Kotlin targets");
    } else if (invokingRule.isSourceAbi()) {
      throw new Error("Source ABI flavor is not supported for Kotlin targets");
    } else if (!buildClasspathEntries.isEmpty()) {
      addClasspath(builder, buildClasspathEntries);
    }

    if (trackClassUsage) {
      AbsPath depTracker =
          depTrackerPath.orElseThrow(
              () ->
                  new IllegalStateException(
                      "Tracking class usage requires the Kotlin toolchain's"
                          + " track_class_usage_plugin"));
      builder.add(X_PLUGIN_ARG + depTracker);
      builder.add(PLUGIN);
      builder.add(
          "plugin:buck_deps_tracker:out="
              + ruleCellRoot.resolve(getKotlinTempDepFilePath(reportDirPath)));
    }

    if (!extraArguments.isEmpty()) {
      for (String extraArgument : extraArguments) {
        if (!extraArgument.isEmpty()) {
          builder.add(extraArgument);
        }
      }
    }

    if (languageVersion.getSupportsLanguageVersion()) {
      builder.add(languageVersion.getCompilerArgs());
    }

    if (context.getVerbosity().shouldUseVerbosityFlagIfAvailable()
        && !verboseModeOnlyExtraArguments.isEmpty()) {
      for (String extraArgument : verboseModeOnlyExtraArguments) {
        if (!extraArgument.isEmpty()) {
          builder.add(extraArgument);
        }
      }
    }

    return builder.build();
  }

  private void addClasspath(ImmutableList.Builder<String> builder, Iterable<AbsPath> pathElements) {
    builder.add(
        CLASSPATH_FLAG,
        Joiner.on(File.pathSeparator)
            .join(transform(pathElements, path -> path.getPath().toString())));
  }

  /**
   * @return The classpath entries used to invoke javac.
   */
  @VisibleForTesting
  ImmutableList<AbsPath> getClasspathEntries() {
    return combinedClassPathEntries;
  }
}
