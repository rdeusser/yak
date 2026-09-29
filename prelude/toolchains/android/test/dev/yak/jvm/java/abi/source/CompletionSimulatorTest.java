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

import dev.yak.jvm.java.abi.source.CompletionSimulator.CompletedType;
import dev.yak.jvm.java.testutil.compiler.CompilerTreeApiTest;
import java.io.IOException;
import java.util.Map;
import javax.lang.model.element.TypeElement;
import org.hamcrest.Matchers;
import org.junit.Before;
import org.junit.Test;

public class CompletionSimulatorTest extends CompilerTreeApiTest {
  private final TestSourceOnlyAbiRuleInfo ruleInfo = new TestSourceOnlyAbiRuleInfo("//:test");
  private CompletionSimulator completer;

  @Before
  public void setUp() throws IOException {
    withClasspath(SimulatorTestSources.SUPERCLASS);
    withClasspath(SimulatorTestSources.GRAND_SUPERCLASS);
    withClasspath(SimulatorTestSources.INTERFACE1);
    withClasspath(SimulatorTestSources.GRAND_INTERFACE);
    withClasspath(SimulatorTestSources.INTERFACE2);

    ruleInfo.addElementOwner("com.example.superclass.Super", "//com/example/superclass:superclass");
    ruleInfo.addElementOwner(
        "com.example.grandsuper.GrandSuper", "//com/example/grandsuper:grandsuper");
    ruleInfo.addElementOwner("com.example.iface1.Interface1", "//com/example/iface1:iface1");
    ruleInfo.addElementOwner(
        "com.example.grandinterface.GrandInterface", "//com/example/grandinterface:grandinterface");
    ruleInfo.addElementOwner("com.example.iface2.Interface2", "//com/example/iface2:iface2");
    ruleInfo.addElementOwner("com.example.subclass.Subclass", "//com/example/subclass:subclass");
  }

  @Override
  protected void initCompiler(Map<String, String> fileNamesToContents) throws IOException {
    super.initCompiler(fileNamesToContents);

    FileManagerSimulator fileManager = new FileManagerSimulator(elements, trees, ruleInfo);
    completer = new CompletionSimulator(fileManager);
  }

  @Test
  public void testTransitiveCompletionWithAllSupersPresentCompletesSuccessfully()
      throws IOException {
    compile(SimulatorTestSources.SUBCLASS);

    ruleInfo.addAvailableRule("//com/example/superclass:superclass");
    ruleInfo.addAvailableRule("//com/example/grandsuper:grandsuper");
    ruleInfo.addAvailableRule("//com/example/iface1:iface1");
    ruleInfo.addAvailableRule("//com/example/iface2:iface2");
    ruleInfo.addAvailableRule("//com/example/grandinterface:grandinterface");

    CompletedType result = completeSubclass(true);

    assertThat(result.getMissingDependencies(), Matchers.empty());
    assertEquals(CompletedTypeKind.COMPLETED_TYPE, result.kind);
  }

  @Test
  public void testTransitiveMemberClassCompletionWithAllSupersPresentCompletesSuccessfully()
      throws IOException {
    compile(SimulatorTestSources.SUBCLASS);

    ruleInfo.addAvailableRule("//com/example/superclass:superclass");
    ruleInfo.addAvailableRule("//com/example/grandsuper:grandsuper");
    ruleInfo.addAvailableRule("//com/example/iface1:iface1");
    ruleInfo.addAvailableRule("//com/example/iface2:iface2");
    ruleInfo.addAvailableRule("//com/example/grandinterface:grandinterface");

    CompletedType result = completeSubclassMember(true);

    assertThat(result.getMissingDependencies(), Matchers.empty());
    assertEquals(CompletedTypeKind.COMPLETED_TYPE, result.kind);
  }

  @Test
  public void testTransitiveCompletionWithSomeSupersMissingCompletesPartially() throws IOException {
    compile(SimulatorTestSources.SUBCLASS);

    ruleInfo.addAvailableRule("//com/example/grandsuper:grandsuper");
    ruleInfo.addAvailableRule("//com/example/iface1:iface1");
    ruleInfo.addAvailableRule("//com/example/iface2:iface2");
    ruleInfo.addAvailableRule("//com/example/grandinterface:grandinterface");

    CompletedType result = completeSubclass(true);

    assertThat(
        result.getMissingDependencies(),
        Matchers.containsInAnyOrder("//com/example/superclass:superclass"));
    assertEquals(CompletedTypeKind.PARTIALLY_COMPLETED_TYPE, result.kind);
  }

  @Test
  public void testNonTransitiveCompletionWithSomeSupersMissingCompletes() throws IOException {
    compile(SimulatorTestSources.SUBCLASS);

    ruleInfo.addAvailableRule("//com/example/grandsuper:grandsuper");
    ruleInfo.addAvailableRule("//com/example/iface1:iface1");
    ruleInfo.addAvailableRule("//com/example/iface2:iface2");
    ruleInfo.addAvailableRule("//com/example/grandinterface:grandinterface");

    CompletedType result = completeSubclass(false);

    assertThat(result.getMissingDependencies(), Matchers.empty());
    assertEquals(CompletedTypeKind.COMPLETED_TYPE, result.kind);
  }

  @Test
  public void testTransitiveMemberClassCompletionWithSomeOuterSupersMissingCompletesPartially()
      throws IOException {
    compile(SimulatorTestSources.SUBCLASS);

    ruleInfo.addAvailableRule("//com/example/superclass:superclass");
    ruleInfo.addAvailableRule("//com/example/grandsuper:grandsuper");
    ruleInfo.addAvailableRule("//com/example/iface2:iface2");
    ruleInfo.addAvailableRule("//com/example/grandinterface:grandinterface");

    CompletedType result = completeSubclassMember(true);

    assertThat(
        result.getMissingDependencies(),
        Matchers.containsInAnyOrder("//com/example/iface1:iface1"));
    assertEquals(CompletedTypeKind.PARTIALLY_COMPLETED_TYPE, result.kind);
  }

  @Test
  public void testNonTransitiveMemberClassCompletionWithSomeOuterSupersMissingCompletes()
      throws IOException {
    compile(SimulatorTestSources.SUBCLASS);

    ruleInfo.addAvailableRule("//com/example/superclass:superclass");
    ruleInfo.addAvailableRule("//com/example/grandsuper:grandsuper");
    ruleInfo.addAvailableRule("//com/example/iface2:iface2");
    ruleInfo.addAvailableRule("//com/example/grandinterface:grandinterface");

    CompletedType result = completeSubclassMember(false);

    assertThat(result.getMissingDependencies(), Matchers.empty());
    assertEquals(CompletedTypeKind.COMPLETED_TYPE, result.kind);
  }

  @Test
  public void testTransitiveCompletionWithSomeSupersPresentButGrandSupersMissingCrashes()
      throws IOException {
    compile(SimulatorTestSources.SUBCLASS);

    ruleInfo.addAvailableRule("//com/example/superclass:superclass");
    ruleInfo.addAvailableRule("//com/example/iface1:iface1");
    ruleInfo.addAvailableRule("//com/example/iface2:iface2");

    CompletedType result = completeSubclass(true);
    assertThat(
        result.getMissingDependencies(),
        Matchers.containsInAnyOrder(
            "//com/example/grandsuper:grandsuper", "//com/example/grandinterface:grandinterface"));
    assertEquals(CompletedTypeKind.CRASH, result.kind);
  }

  @Test
  public void testNonTransitiveCompletionWithSomeSupersPresentButGrandSupersMissingCompletes()
      throws IOException {
    compile(SimulatorTestSources.SUBCLASS);

    ruleInfo.addAvailableRule("//com/example/superclass:superclass");
    ruleInfo.addAvailableRule("//com/example/iface1:iface1");
    ruleInfo.addAvailableRule("//com/example/iface2:iface2");

    CompletedType result = completeSubclass(false);
    assertThat(result.getMissingDependencies(), Matchers.empty());
    assertEquals(CompletedTypeKind.COMPLETED_TYPE, result.kind);
  }

  @Test
  public void
      testTransitiveMemberClassCompletionWithSomeOuterSupersPresentButGrandSupersMissingCrashes()
          throws IOException {
    compile(SimulatorTestSources.SUBCLASS);

    ruleInfo.addAvailableRule("//com/example/superclass:superclass");
    ruleInfo.addAvailableRule("//com/example/grandsuper:grandsuper");
    ruleInfo.addAvailableRule("//com/example/iface1:iface1");
    ruleInfo.addAvailableRule("//com/example/iface2:iface2");

    CompletedType result = completeSubclassMember(true);
    assertThat(
        result.getMissingDependencies(),
        Matchers.containsInAnyOrder("//com/example/grandinterface:grandinterface"));
    assertEquals(CompletedTypeKind.CRASH, result.kind);
  }

  @Test
  public void
      testNonTransitiveMemberClassCompletionWithSomeOuterSupersPresentButGrandSupersMissingCompletes()
          throws IOException {
    compile(SimulatorTestSources.SUBCLASS);

    ruleInfo.addAvailableRule("//com/example/superclass:superclass");
    ruleInfo.addAvailableRule("//com/example/grandsuper:grandsuper");
    ruleInfo.addAvailableRule("//com/example/iface1:iface1");
    ruleInfo.addAvailableRule("//com/example/iface2:iface2");

    CompletedType result = completeSubclassMember(false);
    assertThat(result.getMissingDependencies(), Matchers.empty());
    assertEquals(CompletedTypeKind.COMPLETED_TYPE, result.kind);
  }

  @Test
  public void testTransitiveCompletionOfMissingDepClassWithSomeSupersMissingReturnsErrorType()
      throws IOException {
    withClasspath(SimulatorTestSources.SUBCLASS);
    initCompiler();

    ruleInfo.addAvailableRule("//com/example/iface1:iface1");
    ruleInfo.addAvailableRule("//com/example/grandinterface:grandinterface");

    CompletedType result = completeSubclass(true);
    assertThat(
        result.getMissingDependencies(),
        Matchers.containsInAnyOrder(
            "//com/example/subclass:subclass",
            "//com/example/superclass:superclass",
            "//com/example/grandsuper:grandsuper",
            "//com/example/iface2:iface2"));
    assertEquals(CompletedTypeKind.ERROR_TYPE, result.kind);
  }

  @Test
  public void testNonTransitiveCompletionOfMissingDepClassWithSomeSupersMissingReturnsErrorType()
      throws IOException {
    withClasspath(SimulatorTestSources.SUBCLASS);
    initCompiler();

    ruleInfo.addAvailableRule("//com/example/iface1:iface1");
    ruleInfo.addAvailableRule("//com/example/grandinterface:grandinterface");

    CompletedType result = completeSubclass(false);
    assertThat(
        result.getMissingDependencies(),
        Matchers.containsInAnyOrder("//com/example/subclass:subclass"));
    assertEquals(CompletedTypeKind.ERROR_TYPE, result.kind);
  }

  @Test
  public void testTransitiveCompletionOfDepClassWithSomeSupersMissingCrashes() throws IOException {
    withClasspath(SimulatorTestSources.SUBCLASS);
    initCompiler();

    ruleInfo.addAvailableRule("//com/example/subclass:subclass");
    ruleInfo.addAvailableRule("//com/example/iface1:iface1");
    ruleInfo.addAvailableRule("//com/example/grandinterface:grandinterface");

    CompletedType result = completeSubclass(true);
    assertThat(
        result.getMissingDependencies(),
        Matchers.containsInAnyOrder(
            "//com/example/superclass:superclass",
            "//com/example/grandsuper:grandsuper",
            "//com/example/iface2:iface2"));
    assertEquals(CompletedTypeKind.CRASH, result.kind);
  }

  @Test
  public void testNonTransitiveCompletionOfDepClassWithSomeSupersMissingReturnsCompletedType()
      throws IOException {
    withClasspath(SimulatorTestSources.SUBCLASS);
    initCompiler();

    ruleInfo.addAvailableRule("//com/example/subclass:subclass");
    ruleInfo.addAvailableRule("//com/example/iface1:iface1");
    ruleInfo.addAvailableRule("//com/example/grandinterface:grandinterface");

    CompletedType result = completeSubclass(false);
    assertThat(result.getMissingDependencies(), Matchers.empty());
    assertEquals(CompletedTypeKind.COMPLETED_TYPE, result.kind);
  }

  private CompletedType completeSubclass(boolean transitive) {
    TypeElement subclass = elements.getTypeElement("com.example.subclass.Subclass");

    return completer.complete(subclass, transitive);
  }

  private CompletedType completeSubclassMember(boolean transitive) {
    TypeElement subclass = elements.getTypeElement("com.example.subclass.Subclass.SubclassMember");

    return completer.complete(subclass, transitive);
  }
}
