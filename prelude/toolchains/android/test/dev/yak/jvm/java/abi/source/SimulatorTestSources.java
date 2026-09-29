/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

package dev.yak.jvm.java.abi.source;

import com.google.common.base.Joiner;
import com.google.common.collect.ImmutableMap;
import java.util.Map;

public class SimulatorTestSources {
  public static final Map<String, String> GRAND_SUPERCLASS =
      ImmutableMap.of(
          "com/example/grandsuper/GrandSuper.java",
          Joiner.on('\n')
              .join(
                  "package com.example.grandsuper;",
                  "public class GrandSuper {",
                  "  public static class GrandSuperMember { }",
                  "}"));

  public static final Map<String, String> SUPERCLASS =
      ImmutableMap.of(
          "com/example/superclass/Super.java",
          Joiner.on('\n')
              .join(
                  "package com.example.superclass;",
                  "import com.example.grandsuper.GrandSuper;",
                  "public class Super extends GrandSuper {",
                  "  public static class SuperMember { }",
                  "}"));

  public static final Map<String, String> SUBCLASS =
      ImmutableMap.of(
          "com/example/subclass/Subclass.java",
          Joiner.on('\n')
              .join(
                  "package com.example.subclass;",
                  "import com.example.superclass.Super;",
                  "import com.example.iface1.Interface1;",
                  "import com.example.iface2.Interface2;",
                  "public class Subclass extends Super implements Interface1, Interface2 {",
                  "  public class SubclassMember extends Super.SuperMember { }",
                  "}"));

  public static final Map<String, String> INTERFACE1 =
      ImmutableMap.of(
          "com/example/iface1/Interface1.java",
          Joiner.on('\n')
              .join(
                  "package com.example.iface1;",
                  "import com.example.grandinterface.GrandInterface;",
                  "public interface Interface1 extends GrandInterface {",
                  "  interface Interface1Member { }",
                  "}"));

  public static final Map<String, String> GRAND_INTERFACE =
      ImmutableMap.of(
          "com/example/grandinterface/GrandInterface.java",
          Joiner.on('\n')
              .join(
                  "package com.example.grandinterface;",
                  "public interface GrandInterface {",
                  "  interface GrandInterfaceMember { }",
                  "}"));

  public static final Map<String, String> INTERFACE2 =
      ImmutableMap.of(
          "com/example/iface2/Interface2.java",
          Joiner.on('\n')
              .join(
                  "package com.example.iface2;",
                  "public interface Interface2 {",
                  "  interface Interface2Member { }",
                  "}"));
}
