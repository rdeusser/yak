/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

package dev.yak.jvm.kotlin.cd.workertool;

import com.google.common.base.Preconditions;
import com.google.common.collect.ImmutableMap;
import com.google.protobuf.util.JsonFormat;
import dev.yak.cd.model.kotlin.Metadata;
import dev.yak.core.filesystems.AbsPath;
import dev.yak.core.filesystems.RelPath;
import dev.yak.core.util.log.Logger;
import dev.yak.io.file.MostFiles;
import dev.yak.jvm.cd.AbiDirWriter;
import dev.yak.jvm.cd.BuildCommandStepsBuilder;
import dev.yak.jvm.cd.DepFileUtils;
import dev.yak.jvm.cd.JvmCDCommand;
import dev.yak.jvm.cd.command.PostBuildParams;
import dev.yak.jvm.cd.command.kotlin.BuildKotlinCommand;
import dev.yak.jvm.cd.serialization.kotlin.ActionMetadataSerializer;
import dev.yak.jvm.java.ActionMetadata;
import dev.yak.jvm.kotlin.KotlinStepsBuilder;
import dev.yak.jvm.kotlin.cd.workertool.postexecutors.ClassAbiWriter;
import dev.yak.jvm.kotlin.cd.workertool.postexecutors.ClassAbiWriterFactory;
import dev.yak.jvm.kotlin.cd.workertool.postexecutors.PreviousStateWriter;
import dev.yak.jvm.kotlin.cd.workertool.postexecutors.PreviousStateWriterFactory;
import dev.yak.jvm.kotlin.cd.workertool.postexecutors.SkippedCompilationTracker;
import dev.yak.jvm.kotlin.cd.workertool.postexecutors.SkippedCompilationTrackerFactory;
import dev.yak.util.zip.ZipScrubber;
import java.io.File;
import java.io.FileReader;
import java.io.IOException;
import java.io.Reader;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import java.util.Optional;
import java.util.StringJoiner;
import java.util.stream.Collectors;
import javax.annotation.Nullable;
import org.kohsuke.args4j.CmdLineException;
import org.kohsuke.args4j.CmdLineParser;
import org.kohsuke.args4j.Option;

/**
 * KotlinCD main class.
 *
 * <p>This provides a simple executable that can run any of the kotlincd actions.
 */
public class KotlinCDCommand implements JvmCDCommand {

  private static final String ACTION_META_DATA_FILE = "action_metadata.json";

  @Option(name = "--action-id", required = true)
  protected String actionId;

  @Option(name = "--command-file", required = true)
  protected Path commandFile;

  @Option(name = "--incremental-config-file")
  protected Path incrementalConfigFile;

  @Option(name = "--logging-level")
  private int loggingLevel = 0;

  private final KotlinStepsBuilder stepsBuilder;
  private final BuildKotlinCommand buildKotlinCommand;
  private final PostBuildParams postBuildParams;
  private final Optional<Path> actionMetadataPath;
  private final Logger logger;
  private final SkippedCompilationTracker kotlinClassesTracker;
  private final SkippedCompilationTracker jvmAbiTracker;

  public KotlinCDCommand(String[] args, ImmutableMap<String, String> env)
      throws CmdLineException, IOException {
    logger = Logger.get(KotlinCDCommand.class.getName());
    CmdLineParser parser = new CmdLineParser(this);
    try {
      parser.parseArgument(args);
    } catch (CmdLineException e) {
      System.err.println(e.getMessage());
      parser.printUsage(System.err);
      throw e;
    }
    actionMetadataPath = initCurrentActionMetadataPath(env);
    dev.yak.cd.model.kotlin.BuildKotlinCommand.Builder builder =
        dev.yak.cd.model.kotlin.BuildKotlinCommand.newBuilder();
    try (Reader reader = new FileReader(commandFile.toFile())) {
      JsonFormat.parser().ignoringUnknownFields().merge(reader, builder);
    }
    RelPath buckScratchPath = RelPath.get(env.get(WORKING_DIRECTORY_ENV_VAR));
    dev.yak.cd.model.kotlin.BuildKotlinCommand proto = builder.build();
    this.buildKotlinCommand =
        BuildKotlinCommand.Companion.fromProto(
            proto.getBuildCommand(), Optional.ofNullable(buckScratchPath));
    this.postBuildParams = PostBuildParams.Companion.fromProto(proto.getPostBuildParams());
    cleanupOldPostBuildOutputs();

    // Capture pre-compilation timestamps for tracking which files skip compilation.
    this.kotlinClassesTracker =
        SkippedCompilationTrackerFactory.create(
            postBuildParams.getFilesWhichSkippedCompilation(),
            buildKotlinCommand.getKotlinExtraParams().getKotlinClassesDir().getPath());
    this.jvmAbiTracker =
        SkippedCompilationTrackerFactory.create(
            postBuildParams.getJvmAbiFilesWhichSkippedCompilation(),
            buildKotlinCommand
                .getKotlinExtraParams()
                .getJvmAbiGenWorkingDir()
                .map(absPath -> absPath.getPath())
                .orElse(null));

    this.stepsBuilder = new KotlinStepsBuilder(this.buildKotlinCommand, generateActionMetadata());
  }

  private Optional<Path> initCurrentActionMetadataPath(ImmutableMap<String, String> env) {
    String actionMetadata = env.get("ACTION_METADATA");
    if (actionMetadata != null) {
      logger.info("ACTION_METADATA: %s", actionMetadata);
      return Optional.of(Paths.get(actionMetadata));
    } else {
      logger.info("ACTION_METADATA is not set");
      return Optional.empty();
    }
  }

  private @Nullable ActionMetadata generateActionMetadata() throws IOException {
    if (!actionMetadataPath.isPresent()) {
      return null;
    }

    dev.yak.cd.model.kotlin.ActionMetadata.Builder builder =
        dev.yak.cd.model.kotlin.ActionMetadata.newBuilder();
    builder.setCurrentMetadata(parseMetadata(actionMetadataPath.get().toFile()));

    Optional<Path> previousActionMetadataPath = getPreviousActionMetadataPath();
    if (previousActionMetadataPath.isPresent()) {
      builder.setPreviousMetadata(
          parseMetadata(previousActionMetadataPath.map(Path::toFile).get()));
    }

    Preconditions.checkNotNull(incrementalConfigFile);
    return ActionMetadataSerializer.deserialize(incrementalConfigFile, builder.build());
  }

  private Optional<Path> getPreviousActionMetadataPath() {
    Preconditions.checkState(postBuildParams.getIncrementalStateDir() != null);
    Path previousMetadataPath =
        postBuildParams.getIncrementalStateDir().resolve(ACTION_META_DATA_FILE);
    if (Files.exists(previousMetadataPath)) {
      logger.info("Previous ACTION_METADATA: %s", previousMetadataPath);
      return Optional.of(previousMetadataPath);
    } else {
      logger.info("Previous ACTION_METADATA does not exist");
      return Optional.empty();
    }
  }

  private static Metadata parseMetadata(File jsonFile) throws IOException {
    Metadata.Builder metadatabuilder = Metadata.newBuilder();

    try (Reader reader = new FileReader(jsonFile)) {
      JsonFormat.parser().ignoringUnknownFields().merge(reader, metadatabuilder);
    }

    return metadatabuilder.build();
  }

  private void cleanupOldPostBuildOutputs() throws IOException {
    if (!buildKotlinCommand.getKotlinExtraParams().getShouldActionRunIncrementally()) {
      return;
    }

    List<Path> oldPostBuildOutputs = new ArrayList<>();
    oldPostBuildOutputs.add(postBuildParams.getLibraryJar());
    oldPostBuildOutputs.add(postBuildParams.getAbiJar());
    oldPostBuildOutputs.add(postBuildParams.getJvmAbiGen());
    oldPostBuildOutputs.add(postBuildParams.getAbiOutputDir());
    oldPostBuildOutputs.addAll(postBuildParams.getUsedClassesPaths());
    oldPostBuildOutputs.addAll(postBuildParams.getOptionalDirsPaths());

    removePaths(oldPostBuildOutputs);
  }

  private static void removePaths(List<Path> paths) throws IOException {
    AbsPath root = AbsPath.of(Paths.get(".").toAbsolutePath().normalize());

    for (Path path : paths) {
      if (path == null) {
        continue;
      }

      if (path.toFile().isDirectory()) {
        MostFiles.deleteRecursivelyIfExists(root.resolve(path));
      } else {
        Files.deleteIfExists(root.resolve(path).getPath());
      }
    }
  }

  protected void maybeWriteClassAbi() {
    if (!postBuildParams.getShouldCreateClassAbi()) {
      Preconditions.checkState(postBuildParams.getJvmAbiGen() == null);
      return;
    }

    ClassAbiWriter classAbiWriter =
        ClassAbiWriterFactory.create(
            buildKotlinCommand.getKotlinExtraParams().getShouldKotlincRunIncrementally(),
            buildKotlinCommand.getKotlinExtraParams().getKotlinClassesDir(),
            buildKotlinCommand.getKotlinExtraParams().getJvmAbiGenWorkingDir().orElse(null),
            postBuildParams.getJvmAbiGen(),
            postBuildParams.getLibraryJar(),
            postBuildParams.getAbiJar());
    classAbiWriter.execute();
  }

  protected void maybeWriteAbiDir() throws IOException {
    if (postBuildParams.getAbiOutputDir() == null) {
      Preconditions.checkState(postBuildParams.getAbiJar() == null);
      return;
    }
    AbiDirWriter.writeAbiOutputDir(postBuildParams.getAbiJar(), postBuildParams.getAbiOutputDir());
  }

  public void maybeWriteDepFile() throws IOException {
    Preconditions.checkState(
        (postBuildParams.getDepFile() == null)
            == (postBuildParams.getUsedClassesPaths().isEmpty()));

    if (postBuildParams.getDepFile() != null) {
      // we won't run javac if not necessary and used classes for java may not exist
      List<Path> usedClassesMapPaths = filterExistingFiles(postBuildParams.getUsedClassesPaths());
      Preconditions.checkState(!usedClassesMapPaths.isEmpty());

      DepFileUtils.usedClassesToDepFile(
          usedClassesMapPaths,
          postBuildParams.getDepFile(),
          Optional.ofNullable(postBuildParams.getJarToJarDirMap()),
          buildKotlinCommand.getKotlinExtraParams().getShouldActionRunIncrementally());
    }
  }

  private static List<Path> filterExistingFiles(List<Path> paths) {
    return paths.stream().filter(Files::exists).collect(Collectors.toList());
  }

  protected void maybeCreateOptionalDirs() throws IOException {
    if (postBuildParams.getOptionalDirsPaths().isEmpty()) {
      return;
    }
    for (Path path : postBuildParams.getOptionalDirsPaths()) {
      if (Files.exists(path)) {
        continue;
      }
      Files.createDirectory(path);
    }
  }

  protected void maybeCreateIncrementalStateDir() throws IOException {
    if (postBuildParams.getIncrementalStateDir() == null) {
      return;
    }
    if (!Files.exists(postBuildParams.getIncrementalStateDir())) {
      Files.createDirectory(postBuildParams.getIncrementalStateDir());
    }
  }

  protected void maybeWritePreviousStateForNextIncrementalRun() {
    PreviousStateWriter previousStateWriter =
        PreviousStateWriterFactory.create(
            buildKotlinCommand.getKotlinExtraParams().getShouldActionRunIncrementally(),
            postBuildParams.getIncrementalStateDir(),
            actionMetadataPath.orElse(null));

    previousStateWriter.execute();
  }

  protected void maybeWriteUsedJarsFile() throws IOException {
    if (postBuildParams.getUsedJarsPath() == null) {
      return;
    }

    List<Path> usedClassesMapPaths =
        postBuildParams.getUsedClassesPaths().stream()
            .filter(Files::exists)
            .collect(Collectors.toList());
    Preconditions.checkState(!usedClassesMapPaths.isEmpty());

    DepFileUtils.usedClassesToUsedJars(
        usedClassesMapPaths,
        postBuildParams.getUsedJarsPath(),
        buildKotlinCommand.getKotlinExtraParams().getShouldActionRunIncrementally());
  }

  @Override
  public BuildCommandStepsBuilder getBuildCommand() {
    return stepsBuilder;
  }

  @Override
  public String getActionId() {
    return actionId;
  }

  @Override
  public int getLoggingLevel() {
    return loggingLevel;
  }

  @Override
  public void postExecute() throws IOException {
    maybeCreateIncrementalStateDir();
    maybeRunPostProcessor();
    maybeWriteClassAbi();
    maybeWriteAbiDir();
    kotlinClassesTracker.writeSkippedFiles();
    jvmAbiTracker.writeSkippedFiles();
    maybeWriteDepFile();
    maybeCreateOptionalDirs();
    maybeWriteUsedJarsFile();
    maybeWritePreviousStateForNextIncrementalRun();
  }

  protected void maybeRunPostProcessor() throws IOException {
    String postProcessorCmd = postBuildParams.getPostProcessorCmd();
    if (postProcessorCmd == null) {
      return;
    }

    Path libraryJar = postBuildParams.getLibraryJar();
    Preconditions.checkNotNull(libraryJar, "libraryJar must be set when postProcessorCmd is set");

    Path tempDir = Files.createTempDirectory("postprocessor");
    try {
      Path postprocessedJar = tempDir.resolve("postprocessed_output.jar");
      List<String> cmd = new ArrayList<>(Arrays.asList(postProcessorCmd.split("\\s+")));
      cmd.add(libraryJar.toString());
      cmd.add(postprocessedJar.toString());

      logger.info("Executing post-processor: %s", String.join(" ", cmd));
      ProcessBuilder pb = new ProcessBuilder(cmd);
      pb.inheritIO();
      Process process = pb.start();
      int exitCode;
      try {
        exitCode = process.waitFor();
      } catch (InterruptedException e) {
        Thread.currentThread().interrupt();
        throw new IOException("Post-processor interrupted", e);
      }
      if (exitCode != 0) {
        throw new IOException("Post-processor command failed with exit code " + exitCode);
      }

      Files.copy(postprocessedJar, libraryJar, java.nio.file.StandardCopyOption.REPLACE_EXISTING);
    } finally {
      MostFiles.deleteRecursivelyIfExists(AbsPath.of(tempDir.toAbsolutePath()));
    }

    // Scrub the jar to normalize timestamps for deterministic builds
    ZipScrubber.scrubZip(libraryJar);
  }

  @Override
  public String toString() {
    return new StringJoiner(", ", KotlinCDCommand.class.getSimpleName() + "[", "]")
        .add("actionId='" + actionId + "'")
        .add("commandFile=" + commandFile)
        .add("loggingLevel=" + loggingLevel)
        .toString();
  }
}
