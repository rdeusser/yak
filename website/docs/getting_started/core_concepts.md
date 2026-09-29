---
id: tutorial_to_concepts
title: From Tutorial to Concepts
---

import TargetDiagram from '@site/src/components/TargetDiagram'; import
TutorialMermaidDiagram from '@site/src/components/TutorialMermaidDiagram';

In the previous tutorials, we’ve created `YAK` files, defined a couple of
targets and successfully built and ran the “Hello World” rust binary and even
wrote and ran a test. Great job done! Now, let’s go through the journey again,
weaving in the core concepts as we go along to see what gets involved during
this learning process.

## Understanding Target Labels

This is one of the most important concepts to understand when using yak. It is
a precise way to identify any buildable unit in your codebase.

In the tutorials, you encountered the following target label like
[`root//yak_lab/greeter_bin:main`](../tutorial_adding_dependencies/#step-5-run-the-binary).

Here is the anatomy of a target label:

<TargetDiagram
    cell_name="root"
    pkg_name="yak_lab/greeter_bin"
    target_name="main"
    cell_href="#cell"
    pkg_href="#package"
    target_href="#target-name"
/>

## Cell

[Cell](../../concepts/key_concepts/#cells) defines a directory tree of one or
more yak [packages](../../concepts/key_concepts/#packages). The root of a yak
cell contains a global configuration file called
[**`.yakconfig`**](../../concepts/yakconfig).

You can run `yak audit cell` to inspect the abs path of each cell root.

## Package

The existence of a [YAK file](#yak-file) (`yak_lab/greeter_bin/YAK`) makes
`yak_lab/greeter_bin` a yak [package](../../concepts/key_concepts/#packages),
which is more than a directory. If a yak target uses the source file as input,
that target is regarded as the **owner** of the source.

## Target name

The name of the target in the package. It is the name we defined in the YAK
file.

```python
rust_binary(
    name = "main",
    srcs = ["main.rs"],
    ...
)
```

It should be unique within the package.

## `YAK` file

In the lab, you’ve already created three
[build files](../../concepts/build_file/):

- `greeter_bin/YAK`
- `greeter_lib/YAK`
- `logging_lib/YAK`

Although configurable, the name of the build file normally is just YAK.

In these YAK files, you’ve written a couple of
[yak targets](../../concepts/build_target/), `:main`, `:library`,
`:logging_lib` and `:test`. yak targets are instances of
[build rules](../../concepts/build_rule/), which defines how the target should
be built. For example, target :main is of rule type
[rust_binary](../../prelude/rules/rust/rust_binary/), the output artifact will
be a binary that’s runnable, while `:library`, `:logging_lib` are of rule type
[rust_library](../../prelude/rules/rust/rust_library/), the output of which will
be a library that can be linked to the binary.

Referring to yak targets in YAK files and CLI commands need to follow a
special [target pattern](../../concepts/target_pattern/), which looks like:

<code>cell//path/to/dir:target</code> or <code>cell//path/to/dir/...</code>

you will soon become very familiar with these patterns during daily development.

#### Tips:

<!--  TODO: change link to yak doc once available for macros -->

- yak targets can be either build rules or
  [macros](https://buck.build/extending/macros.html), which are
  wrappers/extensions around native build rules, macros are usually defined .bzl
  files.
- yak uses [starlark](../../concepts/glossary/#starlark) language which is a
  dialect of python, to define build rules and macros.

## Visualizing Your Tutorial Project

Now that we understand the basic terminology, let's visualize what you built.
We'll start with the simple file structure, then explore how yak interprets
these files as packages and targets.

### File Structure Overview

Here's the complete project structure you built through the tutorials:

<TutorialMermaidDiagram>
{`
graph TD
    A[yak_lab] --> B[greeter_bin/]
    A --> C[greeter_lib/]
    A --> D[logging_lib/]

    B --> B1[YAK]
    B --> B2[src/]
    B2 --> B3[main.rs]

    C --> C1[YAK]
    C --> C2[src/]
    C --> C3[tests/]
    C2 --> C4[lib.rs]
    C3 --> C5[test.rs]

    D --> D1[YAK]
    D --> D2[src/]
    D2 --> D3[lib.rs]

    style A fill:#e1f5fe
    style B fill:#f3e5f5
    style C fill:#e8f5e8
    style D fill:#fff3e0
    style B1 fill:#ffcdd2
    style C1 fill:#ffcdd2
    style D1 fill:#ffcdd2

`} </TutorialMermaidDiagram>

### From Files to Targets

Now that we understand packages and target concepts, let's see how your YAK
files define targets and their relationships:

<TutorialMermaidDiagram>
{`
graph TD
    A["greeter_bin/YAK"] --> A1@{ shape: circle, label: "**🎯 :main**</br> (rust_binary)"}
    B["greeter_lib/YAK"] --> B1@{ shape: circle, label: "**🎯 :library** </br> (rust_library)"}
    B --> B2@{ shape: circle, label: "**🎯 :test** </br> (rust_test)"}
    C["logging_lib/YAK"] --> C1@{ shape: circle, label: "**🎯 :logging_lib** </br> (rust_library)"}

    A1 -.-> A2["srcs: [src/main.rs]"]
    A1 -.-> A3["deps: [:library, :logging_lib]"]

    B1 -.-> B3["srcs: [src/lib.rs]"]
    B1 -.-> B4["visibility: [PUBLIC]"]
    B1 -.-> B5["deps: [:logging_lib]"]

    B2 -.-> B6["srcs: [tests/test.rs]"]
    B2 -.-> B7["deps: [:library]"]

    C1 -.-> C2["srcs: [src/lib.rs]"]
    C1 -.-> C3["visibility: [PUBLIC]"]

    style A fill:#f3e5f5
    style B fill:#f1f8e9
    style C fill:#fff3e0
    style A1 fill:#e1bee7
    style B1 fill:#c8e6c9
    style B2 fill:#c8e6c9
    style C1 fill:#ffe0b2

`} </TutorialMermaidDiagram>

### The Complete Picture

Finally, let's put it all together. This comprehensive diagram shows how your
file structure, yak packages, targets, and dependencies all interconnect to
form a cohesive build system:

<TutorialMermaidDiagram>
{`
graph TD
    A["📁 yak_lab"] --> B["📁 greeter_bin/<br/>(Package)"]
    A --> C["📁 greeter_lib/<br/>(Package)"]
    A --> D["📁 logging_lib/<br/>(Package)"]

    %% greeter_bin package
    B --> B1["📄 YAK"]
    B --> B2["📁 src/"]
    B2 --> B3["📄 main.rs"]
    B1 -.-> B4(("🎯 **:main**<br/>(rust_binary)"))
    B5@{ shape: braces, label: "srcs: main.rs<br/>deps: :library, :logging_lib"}


    %% greeter_lib package
    C --> C1["📄 YAK"]
    C --> C2["📁 src/"]
    C --> C3["📁 tests/"]
    C2 --> C4["📄 lib.rs"]
    C3 --> C5["📄 test.rs"]
    C1 -.-> C6(("🎯 **:library**<br/>(rust_library)"))
    C1 -.-> C7(("🎯 **:test**<br/>(rust_test)"))
    C8@{ shape: braces, label: "srcs: lib.rs<br/>deps: :logging_lib<br/>visibility: PUBLIC"}
    C9@{ shape: braces, label: "srcs: test.rs<br/>deps: :library"}

    %% logging_lib package
    D --> D1["📄 YAK"]
    D --> D2["📁 src/"]
    D2 --> D3["📄 lib.rs"]
    D1 -.-> D4(("🎯 **:logging_lib**<br/>(rust_library)"))
    D5@{ shape: braces, label: "srcs: lib.rs<br/>visibility: PUBLIC"}

    %% Dependencies (shown with solid arrows)
    B4 ==> C6
    B4 ==> D4
    C6 ==> D4
    C7 ==> C6

    subgraph SG1[" "]
      B4
      B5
      B4 ~~~ B5
    end

    subgraph SG2[" "]
      C6
      C8
      C6 ~~~ C8
    end

    subgraph SG3[" "]
      C7
      C9
      C7 ~~~ C9
    end

    subgraph SG4[" "]
      D4
      D5
      D4 ~~~ D5
    end

    %% Styling
    style A fill:#e1f5fe,stroke:#01579b,stroke-width:3px
    style B fill:#f3e5f5,stroke:#4a148c,stroke-width:2px
    style C fill:#e8f5e8,stroke:#1b5e20,stroke-width:2px
    style D fill:#fff3e0,stroke:#e65100,stroke-width:2px

    style B1 fill:#ffcdd2,stroke:#c62828
    style C1 fill:#ffcdd2,stroke:#c62828
    style D1 fill:#ffcdd2,stroke:#c62828

    style B4 fill:#bbdefb,stroke:#1565c0,stroke-width:2px
    style C6 fill:#c8e6c9,stroke:#2e7d32,stroke-width:2px
    style C7 fill:#c8e6c9,stroke:#2e7d32,stroke-width:2px
    style D4 fill:#ffe0b2,stroke:#ef6c00,stroke-width:2px

    classDef fileNode fill:#f5f5f5,stroke:#757575
    classDef targetInfo fill:#fff9c4,stroke:#f57f17,stroke-width:1px
    classDef mainSubgraph fill:#e3f2fd,stroke:#90caf9,stroke-width:1px
    classDef librarySubgraph fill:#f1f8e9,stroke:#a5d6a7,stroke-width:1px
    classDef testSubgraph fill:#f9fbe7,stroke:#c5e1a5,stroke-width:1px
    classDef loggingSubgraph fill:#fff8e1,stroke:#ffcc80,stroke-width:1px

    class B3,C4,C5,D3 fileNode
    class B5,C8,C9,D5 targetInfo

    %% Apply lighter colors to subgraphs corresponding to targets
    style SG1 fill:#e3f2fd,stroke:#90caf9,stroke-width:1px
    style SG2 fill:#f1f8e9,stroke:#a5d6a7,stroke-width:1px
    style SG3 fill:#f9fbe7,stroke:#c5e1a5,stroke-width:1px
    style SG4 fill:#fff8e1,stroke:#ffcc80,stroke-width:1px

`} </TutorialMermaidDiagram>

**Diagram Legend:**

- **Dotted arrows**: Show how YAK files define targets
- **Thick arrows**: Show dependency relationships between targets
- **Double circles**: Represent yak targets with 🎯 icon
- **Curly braces**: Contain target attributes and configurations
- **Subgraphs**: Group targets with their attributes
- **📁 Icons**: Represent directories and packages
- **📄 Icons**: Represent files (YAK files and source files)

## Load Function and Attributes

A `YAK` file can start with one or more load functions, which load macros from
.bzl files for this `YAK` file to use, this is similar to load or include
functions in other programming languages, load function takes the following
syntax: <code>load("@cell//path/to/bzl:some_bzl.bzl", "some_macro")</code>

Each yak target has a set of attributes, which provide powerful ways to define
and customize how the build should be done, you can inspect the
[rule definition](../../prelude/rules/rust/rust_binary/) to see what these
attributes are and the syntax to define them. Some attributes are mandatory and
some are optional with defaults. Most attributes are rule-specific but there are
common ones such as `name`, `srcs`, `deps` and
[visibility](../../concepts/visibility/).

#### Tips:

- Without using load function, yak will default to using native rules with the
  same name;
- One load function can load multiple macros from the same `.bzl` file
- Deps and visibility are important attributes to understand, please read the
  docs!

## Dependencies and Dependency Graph

<TutorialMermaidDiagram>
{`
graph TD
    A(("🎯 **:main**<br/>(rust_binary)")) --> B(("🎯 **:library**<br/>(rust_library)"))
    A --> C(("🎯 **:logging_lib**<br/>(rust_library)"))
    B --> C
    D(("🎯 **:test**<br/>(rust_test)")) --> B

    style A fill:#e3f2fd,stroke:#1976d2,stroke-width:1px
    style B fill:#e8f5e8,stroke:#388e3c,stroke-width:1px
    style C fill:#fff3e0,stroke:#f57c00,stroke-width:1px
    style D fill:#fce4ec,stroke:#c2185b,stroke-width:1px

`} </TutorialMermaidDiagram>

As you craft your program, you may need to rely on other targets, known as
[dependencies](../../concepts/glossary/#dependency). In the "Hello World"
exercise, <code>:library</code> and <code>:logging_lib</code> targets are
dependencies of the <code>:main</code> target. One target can depend on multiple
dependencies, which in turn can have their own dependencies to form a web of
connections, a so-called
[dependency graph](../../concepts/key_concepts/#yaks-dependency-graph), our
lab renders a very simple dependency graph with maximum depth of 2, in real
world, the graph will be much bigger and one top level target could have tens of
thousands dependencies.

Understanding and managing the dependency graph of your yak target is important
for effective development. yak also offers powerful query tools to explore the
dependency graph.

#### Tips:

- Dependency graph size affects build speed and memory usage greatly
- The graph is a DAG graph. So cycles in the dependency graph (circular
  dependency), something like `A->B->...->X->A`, are not allowed, yak will emit
  error when cycle is detected.

## yak commands

In the lab, once yak and source files are in place, we use
`yak build :main --show-output` to build the `:main` target. This uses the
[yak build command](../../users/commands/build/) to compile and link your rust
code into a binary. Now let’s take a closer look at this command. A yak command
is usually composed of a command type ( `build`, `run`, `test` ...), some
[target pattern](../../concepts/target_pattern/), and options. Command options
can offer extra configurations to do the build.

yak accepts multiple targets in one command, such as:

- `yak build target1 target2 target3` builds 3 targets in one command

- `yak build //path/to/dir/...` builds all targets under `path/to/dir`,
  including sub-dir

- `yak build //path/to/dir:` build all targets under package `path/to/dir`,
  without sub packages

- `yak build @a-file` builds targets listed in a-file, which is a plain text
  file

yak builds multiple targets in parallel, unless there are dependencies between
them.

Once target is built, you can use [yak run](../../users/commands/run/),
[yak test](../../users/commands/test) and
[yak install](../../users/commands/install) to test the code.

As you become more adept, you can explore other powerful yak commands, such as:

- [`yak query`](../../users/commands/query/) with various
  [options](../../users/query/uquery/) to analyze the dependency graph, this set
  of commands is by far the most powerful and complicated yak commands to use;
- [`yak kill`](../../users/commands/kill/) to stop running
  [yak daemon](../../concepts/daemon/), this is sometimes needed to recover
  from a failed build due to bad daemon state;
- [`yak clean`](../../users/commands/clean/) to remove build artifacts from
  [yak-out](../../concepts/yak_out/), this is a remedy to recover from failed
  build due to either bad daemon or bad artifacts in cache;
- [`yak log`](../../users/commands/log/) to see information about previous
  builds
- [`yak bxl`](../../bxl/tutorial/) to run bxl scripts. BXL is a yak script
  language using starlark syntax to write complex query or build logic.

#### Tips:

- yak offers help menus for all commands, try the -h option, you can yak -h or
  yak build -h to see all available commands and options

### yak Command Flow

Here's how yak commands work in your tutorial workflow:

<TutorialMermaidDiagram>
{`
graph TD
    A[User runs yak command] --> B{Command Type}

    B -->|build| C[yak build :main]
    B -->|run| D[yak run :main]
    B -->|test| E[yak test :test]

    C --> F[Parse YAK files]
    D --> F
    E --> F

    F --> G[Resolve dependencies]
    G --> H[Execute build actions]
    H --> I[Generate outputs in </br> yak-out/]

    I --> J{Command specific behavior}
    J -->|build| K[Show output path </br> if --show-output available]
    J -->|run| L[Execute binary]
    J -->|test| M[Run tests & show results]

    style A fill:#e1f5fe
    style F fill:#f3e5f5
    style G fill:#e8f5e8
    style H fill:#fff3e0
    style I fill:#fce4ec

`} </TutorialMermaidDiagram>

## Console and Output

During the lab, you’ve seen the console output during build and test process,
these outputs give information about the execution progress and state. yak
offers different kinds of
[consoles](../../users/build_observability/interactive_console/) for various
purposes, and the interactive feature helps during debugging.

## yak-out

So you’ve successfully built the target and run it. Finally, let’s briefly talk
about [yak-out](../../concepts/yak_out/), which is an important concept yet
hard to understand initially. We know that yak builds complicated targets with
big dependency graphs and generates tons of outputs for test and run. Where
should these output artifacts be stored at? The outputs should not be stored at
the source directory which is tracked by the source control system. The outputs
also need to be reused/cached for later builds to save build time. So here comes
yak-out,it has the following characteristics:

- It’s under repo root;
- It has a unique file structure, some directories are hashed for caching
  purpose;
- It stores a lot of data including output artifacts, yak log files, tmp data,
  etc;
- It is managed by yak and developers normally should not manipulate;

#### Tips:

- **Do NOT** delete artifacts manually from yak-out directory and expect yak
  to rebuild them, yak doesn’t track things under yak-out, use yak clean
  instead;
