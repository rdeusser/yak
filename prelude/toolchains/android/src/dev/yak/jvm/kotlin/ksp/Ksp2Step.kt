/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

package dev.yak.jvm.kotlin.ksp

import com.google.common.base.Joiner
import com.google.common.collect.ImmutableList
import com.google.common.collect.ImmutableSortedSet
import com.google.common.collect.Iterables
import com.google.devtools.ksp.impl.KotlinSymbolProcessing
import com.google.devtools.ksp.processing.KSPJvmConfig
import com.google.devtools.ksp.processing.SymbolProcessorProvider
import dev.yak.core.build.execution.context.IsolatedExecutionContext
import dev.yak.core.filesystems.AbsPath
import dev.yak.core.filesystems.RelPath
import dev.yak.core.util.log.Logger
import dev.yak.jvm.cd.command.kotlin.LanguageVersion
import dev.yak.jvm.core.BuildTargetValue
import dev.yak.jvm.java.CompilerOutputPaths
import dev.yak.jvm.kotlin.ksp.incremental.Ksp2Mode
import dev.yak.jvm.kotlin.util.getExpandedSourcePaths
import dev.yak.step.StepExecutionResult
import dev.yak.step.StepExecutionResults
import dev.yak.step.isolatedsteps.IsolatedStep
import dev.yak.util.CapturingPrintStream
import java.io.File
import java.io.IOException
import java.io.PrintStream
import java.net.URLClassLoader
import java.nio.charset.StandardCharsets
import java.nio.file.Path
import java.util.Optional
import java.util.ServiceLoader
import kotlin.time.measureTimedValue

class Ksp2Step(
    private val invokingRule: BuildTargetValue,
    private val outputPaths: CompilerOutputPaths,
    private val rootPath: AbsPath,
    private val shouldTrackClassUsage: Boolean,
    private val allClasspaths: ImmutableList<AbsPath>,
    private val kotlinPluginGeneratedOutFullPath: String,
    private val annotationProcessorParams: ImmutableSortedSet<String>,
    private val sourceFilePaths: ImmutableSortedSet<RelPath>,
    private val kspDepFilePath: RelPath,
    private val moduleName: String,
    private val kspProcessorsClasspathList: List<String>,
    private val kspClassesOutput: RelPath,
    private val kspKotlinOutput: RelPath,
    private val kspJavaOutput: RelPath,
    private val kspOutputBaseDir: RelPath,
    private val jvmTarget: Optional<String>,
    private val languageVersion: LanguageVersion,
    private val jvmDefaultMode: String,
    private val javaBinary: Optional<String>,
    private val ksp2Mode: Ksp2Mode,
) : IsolatedStep {

  @Throws(IOException::class, InterruptedException::class)
  override fun executeIsolatedStep(context: IsolatedExecutionContext): StepExecutionResult {
    CapturingPrintStream().use { stderr ->
      try {
        val (exitCode, elapsed) = measureTimedValue { executeKsp2(stderr, context) }
        val durationMs = elapsed.inWholeMilliseconds
        // Same shape KotlincStep already emits, so the two steps are greppable together.
        LOG.info(
            "KOTLINCD_STEP_DURATION|%s|%s|%d|%d",
            invokingRule.fullyQualifiedName,
            this::class.java.simpleName,
            durationMs,
            sourceFilePaths.size,
        )
        return when (exitCode) {
          KotlinSymbolProcessing.ExitCode.OK -> StepExecutionResults.SUCCESS
          KotlinSymbolProcessing.ExitCode.PROCESSING_ERROR ->
              StepExecutionResult(
                  StepExecutionResults.ERROR_EXIT_CODE,
                  Optional.of(stderr.getContentsAsString(StandardCharsets.UTF_8)),
              )
        }
      } catch (e: LinkageError) {
        return StepExecutionResult(
            StepExecutionResults.ERROR_EXIT_CODE,
            Optional.of(
                "${stderr.getContentsAsString(StandardCharsets.UTF_8)}\n${e.stackTraceToString()}For a URLClassLoader LinkageError, try adding the affected class to FilteringClassLoader's allowlist.",
            ),
        )
      } catch (e: Throwable) {
        return StepExecutionResult(
            StepExecutionResults.ERROR_EXIT_CODE,
            Optional.of(
                "${stderr.getContentsAsString(StandardCharsets.UTF_8)}\n${e.stackTraceToString()}",
            ),
        )
      }
    }
  }

  private fun executeKsp2(
      stdErr: PrintStream,
      context: IsolatedExecutionContext,
  ): KotlinSymbolProcessing.ExitCode {
    val logger = BuckKsp2Logger(stdErr)

    // Load processors
    val processorClassloader: ClassLoader = URLClassLoader(
        kspProcessorsClasspathList.map { File(it).toURI().toURL() }.toTypedArray(),
        filteringClassLoader,
    )
    val processorProviders =
        ServiceLoader.load(
                processorClassloader.loadClass(
                    "com.google.devtools.ksp.processing.SymbolProcessorProvider",
                ),
                processorClassloader,
            )
            .toList() as List<SymbolProcessorProvider>

    // Build processor options
    val apOptions = getApOptions()

    // Expand source paths
    val sourceFilePathsExpanded = getExpandedSourcePathsOrThrow(
        ruleCellRoot = context.ruleCellRoot,
        kotlinSourceFilePaths = sourceFilePaths,
        workingDirectory = Optional.of(outputPaths.workingDirectory.getPath()),
        invokingRule = invokingRule,
        logger = logger,
    )
    val sourceFilePathsResolved = sourceFilePathsExpanded.map { rootPath.resolve(it).toFile() }

    // Build KSP config
    val kspConfig =
        KSPJvmConfig.Builder()
            .apply {
              // All configurations happen here. See [KSPConfig] for all available options.
              projectBaseDir = rootPath.toFile()
              classOutputDir = rootPath.resolve(kspClassesOutput).toFile()
              kotlinOutputDir = rootPath.resolve(kspKotlinOutput).toFile()
              javaOutputDir = rootPath.resolve(kspJavaOutput).toFile()
              resourceOutputDir = rootPath.resolve(kspClassesOutput).toFile()
              outputBaseDir = rootPath.resolve(kspOutputBaseDir).toFile()
              processorOptions = apOptions
              moduleName = this@Ksp2Step.moduleName
              jvmTarget = this@Ksp2Step.jvmTarget.orElse("1.8")
              sourceRoots = sourceFilePathsResolved
              javaSourceRoots = sourceFilePathsResolved
              languageVersion = this@Ksp2Step.languageVersion.value
              apiVersion = this@Ksp2Step.languageVersion.value
              libraries = allClasspaths.map { it.toFile() }
              jvmDefaultMode = this@Ksp2Step.jvmDefaultMode
              resolveJdkHome(this@Ksp2Step.javaBinary)?.let { jdkHome = it }

              when (ksp2Mode) {
                is Ksp2Mode.NonIncremental -> {
                  cachesDir = rootPath.resolve(ksp2Mode.kspCachesOutput).toFile()
                }
                is Ksp2Mode.Incremental -> {
                  incremental = true
                  cachesDir = ksp2Mode.cachesDir.toFile()
                  incrementalLog = ksp2Mode.incrementalLog
                  modifiedSources = ksp2Mode.modifiedSources.map { it.toFile() }
                  removedSources = ksp2Mode.removedSources.map { it.toFile() }
                  changedClasses = ksp2Mode.changedClasses
                }
              }
            }
            .build()

    logger.info(
        """Running KSP2 with
              |processors: ${processorProviders.joinToString(", ") { it::class.java.name }}
              |KSPJvmConfig: [
              |  projectBaseDir = ${kspConfig.projectBaseDir}
              |  classOutputDir = ${kspConfig.classOutputDir}
              |  kotlinOutputDir = ${kspConfig.kotlinOutputDir}
              |  javaOutputDir = ${kspConfig.javaOutputDir}
              |  resourceOutputDir = ${kspConfig.resourceOutputDir}
              |  cachesDir = ${kspConfig.cachesDir}
              |  outputBaseDir = ${kspConfig.outputBaseDir}
              |  processorOptions = [
              |    ${kspConfig.processorOptions.map { it.key + "=" + it.value }.joinToString(",\n    ")}]
              |  moduleName = ${kspConfig.moduleName}
              |  jvmTarget = ${kspConfig.jvmTarget}
              |  sourceRoots = ${kspConfig.sourceRoots}
              |  javaSourceRoots = ${kspConfig.javaSourceRoots}
              |  languageVersion = ${kspConfig.languageVersion}
              |  apiVersion = ${kspConfig.apiVersion}
              |  libraries = ${kspConfig.libraries}
              |  jvmDefaultMode = ${kspConfig.jvmDefaultMode}
              |  jdkHome = ${kspConfig.jdkHome}
              |  incremental = ${kspConfig.incremental}
              |  incrementalLog = ${kspConfig.incrementalLog}
              |  modifiedSources = ${kspConfig.modifiedSources.joinToString()}
              |  removedSources = ${kspConfig.removedSources.joinToString()}
              |  changedClasses = ${kspConfig.changedClasses.joinToString()}
              |]"""
            .trimMargin(),
    )
    // Run!
    val kotlinSymbolProcessing =
        KotlinSymbolProcessing(kspConfig, processorProviders, logger)
    return kotlinSymbolProcessing.execute()
  }

  private fun getApOptions(): MutableMap<String, String> {
    val apOptions = mutableMapOf<String, String>()

    // KSP needs the full classpath in order to resolve resources
    val allClasspath =
        Joiner.on(File.pathSeparator)
            .join(Iterables.transform(allClasspaths) { path: AbsPath -> path.getPath().toString() })
    apOptions["cp"] = allClasspath.replace(',', '-')

    if (shouldTrackClassUsage) {
      apOptions["fileAccessHistoryReportFile"] = rootPath.resolve(kspDepFilePath).toString()
    }

    if (invokingRule.isSourceOnlyAbi) {
      apOptions["dev.yak.kotlin.generating_abi"] = "true"
    }

    for (param: String in annotationProcessorParams) {
      val paramAndValue = param.split("=".toRegex()).dropLastWhile { it.isEmpty() }.toTypedArray()
      check(paramAndValue.size == 2)
      apOptions[paramAndValue[0]] = paramAndValue[1]
    }

    apOptions["dev.yak.kotlin.ksp_generated_out_path"] = kotlinPluginGeneratedOutFullPath
    return apOptions
  }

  override fun getIsolatedStepDescription(context: IsolatedExecutionContext): String {
    return "Running ksp2 for $moduleName"
  }

  override fun getShortName(): String {
    return "ksp2"
  }

  private fun getExpandedSourcePathsOrThrow(
      ruleCellRoot: AbsPath,
      kotlinSourceFilePaths: ImmutableSortedSet<RelPath>,
      workingDirectory: Optional<Path>,
      invokingRule: BuildTargetValue,
      logger: BuckKsp2Logger,
  ) =
      try {
        getExpandedSourcePaths(ruleCellRoot, kotlinSourceFilePaths, workingDirectory)
      } catch (exception: IOException) {
        logger.exception(exception)
        throw RuntimeException(
            "Unable to expand sources for ${invokingRule.fullyQualifiedName} into $workingDirectory",
        )
      }

  companion object {
    private val LOG: Logger = Logger.get(Ksp2Step::class.java)

    private val jdkHomeCache = java.util.concurrent.ConcurrentHashMap<String, File>()
    private val JAVA_HOME_REGEX = Regex("""java\.home\s*=\s*(.+)""")

    /**
     * Resolves the JDK home directory from the declared java binary input.
     *
     * KSP2 needs jdkHome to resolve JDK types (java.lang.AutoCloseable, etc.) for JVM targets.
     * Android targets use android.jar instead and do not need jdkHome — returns null when
     * javaBinary is absent so callers can skip setting jdkHome (KSP2 defaults to
     * System.getProperty("java.home")).
     *
     * Similar to compile_kotlin.py's _get_jdk_home() which uses jdk_locator, this invokes the java
     * binary with -XshowSettings:properties to discover java.home at action time. The java binary
     * is a declared Buck2 input (from java_toolchain.java), making this hermetic.
     *
     * Result is cached per java binary path (subprocess runs at most once per distinct binary).
     */
    private fun resolveJdkHome(javaBinary: Optional<String>): File? {
      val binary = javaBinary.filter { it.isNotEmpty() }.orElse(null) ?: return null
      return jdkHomeCache.computeIfAbsent(binary) { resolveJdkHomeUncached(it) }
    }

    private fun resolveJdkHomeUncached(binary: String): File {
      val process =
          ProcessBuilder(binary, "-XshowSettings:properties", "-version")
              .redirectErrorStream(true)
              .start()
      // Drain the pipe in a background thread to avoid deadlock: the process can
      // write more than the OS pipe buffer (~64KB), blocking if no one reads.
      // Meanwhile waitFor() provides the timeout guarantee.
      val outputFuture =
          java.util.concurrent.CompletableFuture.supplyAsync {
            process.inputStream.bufferedReader().readText()
          }
      val completed = process.waitFor(60, java.util.concurrent.TimeUnit.SECONDS)
      if (!completed) {
        process.destroyForcibly()
        throw RuntimeException("Timed out resolving JDK home from java binary: $binary")
      }
      val exitCode = process.exitValue()
      val output = outputFuture.get()
      if (exitCode != 0) {
        throw RuntimeException(
            "java binary exited with code $exitCode: $binary\n" + "Output: ${output.take(500)}",
        )
      }
      val match = JAVA_HOME_REGEX.find(output)
      val path =
          match?.groupValues?.get(1)?.trim()
              ?: throw RuntimeException(
                  "Could not parse java.home from java binary output: $binary\n" +
                      "Output (first 500 chars): ${output.take(500)}",
              )
      return File(path)
    }

    private val filteringClassLoader = FilteringClassLoader(
        this::class.java.classLoader,
        ClassLoader.getPlatformClassLoader(),
        "com.google.devtools.ksp.",
        "kotlin.",
        "ksp.",
    )
  }
}
