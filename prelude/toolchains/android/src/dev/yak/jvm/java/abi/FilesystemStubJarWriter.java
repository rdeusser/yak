/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

package dev.yak.jvm.java.abi;

import dev.yak.core.filesystems.AbsPath;
import dev.yak.io.pathformat.PathFormatter;
import dev.yak.util.function.ThrowingSupplier;
import dev.yak.util.zip.CustomZipEntry;
import dev.yak.util.zip.JarBuilder;
import dev.yak.util.zip.JarEntrySupplier;
import java.io.IOException;
import java.io.InputStream;
import java.nio.file.Path;

/** A {@link StubJarWriter} that writes to a file. */
class FilesystemStubJarWriter implements StubJarWriter {

  private final AbsPath outputPath;
  private final JarBuilder jarBuilder;
  private boolean closed = false;

  public FilesystemStubJarWriter(AbsPath outputPath) {
    this.outputPath = outputPath;
    this.jarBuilder = new JarBuilder().setShouldHashEntries(true).setShouldMergeManifests(true);
  }

  @Override
  public void writeEntry(
      Path relativePath, ThrowingSupplier<InputStream, IOException> streamSupplier) {
    jarBuilder.addEntry(
        new JarEntrySupplier(
            new CustomZipEntry(PathFormatter.pathWithUnixSeparators(relativePath)),
            streamSupplier));
  }

  @Override
  public void close() throws IOException {
    if (!closed) {
      jarBuilder.createJarFile(outputPath.getPath());
    }
    closed = true;
  }
}
