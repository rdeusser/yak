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

import static org.hamcrest.MatcherAssert.assertThat;
import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertSame;

import com.google.common.base.Joiner;
import com.sun.source.tree.VariableTree;
import com.sun.source.util.TreePath;
import dev.yak.jvm.java.testutil.compiler.CompilerTreeApiTest;
import java.io.IOException;
import java.util.Map;
import javax.lang.model.element.TypeElement;
import javax.lang.model.element.VariableElement;
import javax.lang.model.util.ElementFilter;
import org.hamcrest.Matchers;
import org.junit.Before;
import org.junit.Test;
import org.junit.runner.RunWith;
import org.junit.runners.Parameterized;
import org.junit.runners.Parameterized.Parameter;
import org.junit.runners.Parameterized.Parameters;

@RunWith(Parameterized.class)
public class CompilerTypeResolutionSimulatorTest extends CompilerTreeApiTest {
  @Parameters(name = "{0}")
  public static Object[] getParameters() {
    return ResolvedTypeKind.values();
  }

  @Parameter public ResolvedTypeKind kind;

  private final TestSourceOnlyAbiRuleInfo ruleInfo = new TestSourceOnlyAbiRuleInfo("//:test");
  private CompilerTypeResolutionSimulator resolver;

  @Before
  public void setUp() throws IOException {
    withClasspath(SimulatorTestSources.SUBCLASS);
    withClasspath(SimulatorTestSources.SUPERCLASS);
    withClasspath(SimulatorTestSources.GRAND_SUPERCLASS);
    withClasspath(SimulatorTestSources.INTERFACE1);
    withClasspath(SimulatorTestSources.GRAND_INTERFACE);
    withClasspath(SimulatorTestSources.INTERFACE2);

    ruleInfo.addElementOwner("com.example.subclass.Subclass", "//com/example/subclass:subclass");
    ruleInfo.addElementOwner("com.example.superclass.Super", "//com/example/superclass:superclass");
    ruleInfo.addElementOwner(
        "com.example.grandsuper.GrandSuper", "//com/example/grandsuper:grandsuper");
    ruleInfo.addElementOwner("com.example.iface1.Interface1", "//com/example/iface1:iface1");
    ruleInfo.addElementOwner(
        "com.example.grandinterface.GrandInterface", "//com/example/grandinterface:grandinterface");
    ruleInfo.addElementOwner("com.example.iface2.Interface2", "//com/example/iface2:iface2");
    ruleInfo.addElementOwner("com.example.subclass.Subclass", "//com/example/subclass:subclass");

    ruleInfo.addAvailableRule("//com/example/superclass:superclass");
    ruleInfo.addAvailableRule("//com/example/iface1:iface1");
    ruleInfo.addAvailableRule("//com/example/iface2:iface2");
    ruleInfo.addAvailableRule("//com/example/grandinterface:grandinterface");

    switch (kind) {
      case RESOLVED_TYPE:
        ruleInfo.addAvailableRule("//com/example/subclass:subclass");
        ruleInfo.addAvailableRule("//com/example/grandsuper:grandsuper");
        break;
      case ERROR_TYPE:
        break;
      case CRASH:
        ruleInfo.addAvailableRule("//com/example/subclass:subclass");
        break;
    }
  }

  @Override
  protected void initCompiler(Map<String, String> fileNamesToContents) throws IOException {
    super.initCompiler(fileNamesToContents);

    FileManagerSimulator fileManager = new FileManagerSimulator(elements, trees, ruleInfo);
    resolver = new CompilerTypeResolutionSimulator(trees, fileManager);
  }

  @Test
  public void testResolvingTypeFromDependencies() throws IOException {
    compile(
        Joiner.on('\n')
            .join(
                "public class Foo {",
                "  com.example.subclass.Subclass.GrandSuperMember field;",
                "}"));

    ResolvedType result = resolveField();

    assertSame(
        elements.getTypeElement("com.example.grandsuper.GrandSuper.GrandSuperMember"), result.type);
    assertEquals(kind, result.kind);

    if (kind == ResolvedTypeKind.ERROR_TYPE) {
      assertThat(
          result.missingDependencies,
          Matchers.containsInAnyOrder(
              "//com/example/subclass:subclass", "//com/example/grandsuper:grandsuper"));
    } else if (kind == ResolvedTypeKind.CRASH) {
      assertThat(
          result.missingDependencies,
          Matchers.containsInAnyOrder("//com/example/grandsuper:grandsuper"));
    }
  }

  @Test
  public void testResolvingTypeViaLocalSubclass() throws IOException {
    if (kind == ResolvedTypeKind.ERROR_TYPE) {
      // It isn't actually possible to get an error type in this situation, because once you've
      // started traversing enclosing classes, you've got to be able to traverse them all.
      return;
    }

    compile(
        Joiner.on('\n')
            .join(
                "public class Foo {",
                "  Other.GrandSuperMember field;",
                "}",
                "class Other extends com.example.subclass.Subclass {",
                "}"));

    ResolvedType result = resolveField();

    assertSame(
        elements.getTypeElement("com.example.grandsuper.GrandSuper.GrandSuperMember"), result.type);
    assertEquals(kind, result.kind);

    if (kind == ResolvedTypeKind.ERROR_TYPE) {
      assertThat(
          result.missingDependencies,
          Matchers.containsInAnyOrder(
              "//com/example/subclass:subclass", "//com/example/grandsuper:grandsuper"));
    } else if (kind == ResolvedTypeKind.CRASH) {
      assertThat(
          result.missingDependencies,
          Matchers.containsInAnyOrder("//com/example/grandsuper:grandsuper"));
    }
  }

  private ResolvedType resolveField() {
    TypeElement fooType = elements.getTypeElement("Foo");
    VariableElement field = ElementFilter.fieldsIn(fooType.getEnclosedElements()).get(0);
    TreePath fieldTreePath = trees.getPath(field);
    VariableTree fieldTree = (VariableTree) fieldTreePath.getLeaf();
    TreePath fieldTypePath = new TreePath(fieldTreePath, fieldTree.getType());

    return resolver.resolve(fieldTypePath);
  }
}
