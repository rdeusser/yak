/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

package dev.yak.jvm.java;

import com.google.common.collect.ImmutableMap;
import dev.yak.core.filesystems.AbsPath;
import dev.yak.core.filesystems.RelPath;
import java.nio.file.Path;
import java.util.Set;

public interface ClassUsageFileWriter {

  void writeFile(
      ImmutableMap<Path, Set<Path>> classUsages,
      RelPath relativePath,
      AbsPath rootPath,
      RelPath configuredBuckOut);
}
