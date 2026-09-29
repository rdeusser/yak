/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

package dev.yak.core.build.execution.context

import com.google.common.io.Closer
import dev.yak.core.filesystems.AbsPath
import dev.yak.util.ClassLoaderCache
import dev.yak.util.Console
import dev.yak.util.Verbosity
import java.io.Closeable
import java.io.IOException
import java.io.PrintStream
import java.util.Optional

/** The context exposed for executing `IsolatedStep`s */
data class IsolatedExecutionContext(
    val classLoaderCache: ClassLoaderCache,
    val console: Console,
    val ruleCellRoot: AbsPath,
) : Closeable {
  val verbosity: Verbosity
    get() = console.verbosity

  val stdErr: PrintStream
    get() = console.stdErr

  val stdOut: PrintStream
    get() = console.stdErr

  @Throws(IOException::class)
  override fun close() {
    // Using a Closer makes it easy to ensure that exceptions from one of the closeables don't
    // cancel the others.
    Closer.create().use { closer -> registerCloseables(closer) }
  }

  protected fun registerCloseables(closer: Closer) {
    closer.register(Closeable { classLoaderCache.close() })
  }

  /** Creates SubContext */
  fun createSubContext(
      newStdout: PrintStream?,
      newStderr: PrintStream?,
      verbosityOverride: Optional<Verbosity?>,
  ): IsolatedExecutionContext {
    val console = this.console
    val newConsole = Console(verbosityOverride.orElse(console.verbosity), newStdout, newStderr)

    // This should replace (or otherwise retain) all of the closeable parts of the context.
    return IsolatedExecutionContext(
        classLoaderCache.addRef(),
        newConsole,
        ruleCellRoot,
    )
  }

  companion object {
    /** Returns an [IsolatedExecutionContext]. */
    @JvmStatic
    fun of(
        classLoaderCache: ClassLoaderCache,
        console: Console,
        ruleCellRoot: AbsPath,
    ): IsolatedExecutionContext {
      return IsolatedExecutionContext(
          classLoaderCache.addRef(),
          console,
          ruleCellRoot,
      )
    }
  }
}
