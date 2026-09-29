/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

package dev.yak.android.dex;

import static org.junit.Assert.assertFalse;
import static org.junit.Assert.assertTrue;

import com.google.common.collect.ImmutableList;
import org.junit.Test;

public class ClassNameFilterTest {
  @Test
  public void testFiltering() {
    ClassNameFilter filter =
        ClassNameFilter.fromConfiguration(
            ImmutableList.of(
                "^org/acra/",
                "^org/tukaani/",
                "/FbInjector^",
                "^com/example/build/Config^",
                "/nodex/",
                // regex patterns
                "^-com\\/example\\/intent\\$(FbrpcIntent|ChooserActivityIntent)$",
                "^-^com\\/example\\/.*\\/util"));

    assertTrue(filter.matches("org/acra/Reporter"));
    assertTrue(filter.matches("org/tukaani/Decoder$State"));
    assertTrue(filter.matches("com/example/inject/FbInjector"));
    assertTrue(filter.matches("com/example/build/Config"));
    assertTrue(filter.matches("com/example/nodex/Splash"));
    assertFalse(filter.matches("borg/acra/Reporter"));
    assertFalse(filter.matches("worg/tukaani/Decoder"));
    assertFalse(filter.matches("com/example/inject/FbInjectorImpl"));
    assertFalse(filter.matches("com/example/inject/FbInjector^"));
    assertFalse(filter.matches("com/example/build/Configs"));
    assertFalse(filter.matches("dcom/example/build/Config"));
    assertFalse(filter.matches("com/example/fake/build/Config"));
    assertFalse(filter.matches("com/example/modex/Splash"));
    // Test cases for regex match
    assertTrue(filter.matches("com/example/intent$FbrpcIntent"));
    assertFalse(filter.matches("com/example/intent$FbrpcIntent/Config"));
    assertTrue(filter.matches("com/example/intent$ChooserActivityIntent"));
    assertTrue(filter.matches("/com/example/intent$ChooserActivityIntent"));
    assertTrue(filter.matches("com/example/intent/local/utility/store"));
    assertFalse(filter.matches("com/example/intent/local/store"));
    assertFalse(filter.matches("/com/example/whatever/util/whatever"));
  }
}
