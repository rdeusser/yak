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
import dev.yak.jvm.core.BuildTargetValue;

/** {@link BuildTargetValue} to protobuf serializer */
@Nullsafe(Nullsafe.Mode.LOCAL)
public class BuildTargetValueSerializer {

  private BuildTargetValueSerializer() {}

  /**
   * Deserializes javacd model's {@link dev.yak.cd.model.java.BuildTargetValue} into {@link
   * BuildTargetValue}.
   */
  public static BuildTargetValue deserialize(
      dev.yak.cd.model.java.BuildTargetValue buildTargetValue) {
    return new BuildTargetValue(
        buildTargetValue.getType(), buildTargetValue.getFullyQualifiedName());
  }
}
