---
id: concept_map
title: Concept Map
---

The Concept Map provides an at-a-glance overview of the relationships between
widely used yak concepts. It is meant to be a tool to help those onboarding to
yak to quickly gain an understanding of the yak environment.

```mermaid
flowchart TB
  daemon["Daemon"] -->|1 and only 1 per| isolation["Isolation dir"]
  isolation -->|default is 1 per| project["Project (aka root cell)"]
  project -->|defined by| yakconfigs[".yakconfig files"]
  project -->|contains| cells["Cells"]
  yakconfigs -->|specify| configs["Configs"]
  yakconfigs -->|specify| cells
  cells -->|contain| cells
  cells -->|contain| packages["Packages"]
  buildfiles["Build files"] -->|1 and only 1 per| packages
  buildfiles -->|defines| targets["Targets"]
  buildfiles -->|written in| starlark["Starlark (BXL)"]
  starlark -->|can read| packagefiles["PACKAGE files"]
  rules["Rules"] -->|written in| starlark
  rules -->|declare| attributes["Attributes"]
  attributes -->|set in| targets
  targets -->|instances of| rules
  targets -->|have other targets as| dependencies["Dependencies"]
  execdeps["Exec deps"] -->|types of| dependencies
  toolchaindeps["Toolchain deps"] -->|types of| dependencies
  targetdeps["Target deps"] -->|types of| dependencies
  configurationdeps["Configuration deps"] -->|types of| dependencies
  dependencies -->|form| unconfiguredgraph["Unconfigured graph"]
  targets -->|identified via| labels["Target labels"]
  patterns["Target patterns"] -->|describe a set of| labels
  transition["Transition"] -->|change| configurations["Configurations"]
  configurations -->|applied to| targets
  configurations -->|have| constraints["Constraints"]
  configurations -->|applied via| selects["Selects"]
  selects -->|form| configuredgraph["Configured target graph"]
  constraints -->|define| platforms["Platforms"]
  platforms -->|are| targets
  toolchains["Toolchains"] -->|are| targets
  execplatforms["Execution platforms"] -->|are| targets
  execplatforms -->|have one| platforms
  execplatforms -->|define support for| machines["Machines"]
  machines -->|can be| local["Local"]
  machines -->|can be| remote["Remote"]
  actions["Actions"] -->|run on| machines
  rules -->|output info into other rules via| providers["Providers"]
  subtargets["Subtargets"] -->|collection of| providers
  rules -->|create| actions
  actions -->|form| actiongraph["Action graph"]
  configuredgraph -->|defines| actiongraph
  artifacts["Artifacts"] -->|inputs| actions
  actions -->|outputs| artifacts
  sourcefiles["Source files"] -->|type of| artifacts
  packages -->|contain| sourcefiles
  artifacts -->|might live in| yakout["yak-out"]
  binaries["Binaries"] -->|are| artifacts
  libraries["Libraries"] -->|are| artifacts
  builds["Builds"] -->|build| binaries
  builds -->|build| libraries
  builds -->|invoked via| labels
```

:::note

The Concept Map is for reference only and is not intended to be 100% accurate
nor complete.

:::
