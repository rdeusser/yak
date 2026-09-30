/*
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Selects the targets that the changes since a Git revision can affect, for
//! `yak test --changed-since`.
//!
//! The selection reads the current target graph only. A package is changed when its evaluation
//! can differ from its evaluation at the merge base: its build file or a `PACKAGE` file above it
//! changed, a Starlark file it loads changed, a file appeared in or disappeared from it, or it
//! belongs to a cargo cell whose `cargo metadata` output can differ. A target is changed when its
//! package is changed or one of its inputs changed. A target is affected when it or a target it
//! depends on is changed, where the dependencies include configuration dependencies, the target
//! platform, and the execution platforms.

use std::sync::Arc;

use dice::DiceTransaction;
use dupe::Dupe;
use futures::TryStreamExt;
use futures::future::join_all;
use yak_cli_proto::ChangedFiles;
use yak_common::buildfiles::HasBuildfiles;
use yak_common::dice::cells::HasCellResolver;
use yak_common::legacy_configs::dice::HasLegacyConfigs;
use yak_common::legacy_configs::key::YakconfigKeyRef;
use yak_common::pattern::package_roots::find_package_roots_stream;
use yak_common::pattern::resolve::ResolvedPattern;
use yak_core::bzl::ImportPath;
use yak_core::cells::CellResolver;
use yak_core::cells::cell_path::CellPath;
use yak_core::cells::external::ExternalCellOrigin;
use yak_core::cells::name::CellName;
use yak_core::cells::paths::CellRelativePath;
use yak_core::fs::project_rel_path::ProjectRelativePath;
use yak_core::fs::project_rel_path::ProjectRelativePathBuf;
use yak_core::package::PackageLabel;
use yak_core::pattern::pattern::PackageSpec;
use yak_core::pattern::pattern_type::ConfiguredProvidersPatternExtra;
use yak_core::target::label::label::TargetLabel;
use yak_fs::paths::file_name::FileName;
use yak_fs::paths::file_name::FileNameBuf;
use yak_hash::YakMutMap;
use yak_hash::YakMutSet;
use yak_interpreter::load_module::INTERPRETER_CALCULATION_IMPL;
use yak_interpreter::load_module::InterpreterCalculation;
use yak_interpreter::paths::package::PackageFilePath;
use yak_node::configuration::target_platform_detector::TargetPlatformDetector;
use yak_node::execution::EXECUTION_PLATFORMS_YAKCONFIG;
use yak_node::nodes::eval_result::EvaluationResult;
use yak_node::nodes::frontend::TargetGraphCalculation;
use yak_node::nodes::unconfigured::TargetNode;
use yak_node::nodes::unconfigured::TargetNodeRef;

/// Files that select every test when they change. rustup picks the compiler of every Rust action
/// from these files in the project root, and no action declares them as inputs.
const SELECT_ALL_FILES: &[&str] = &["rust-toolchain", "rust-toolchain.toml"];

/// Project-relative paths, set in `[test] changed_since_select_all`, that select every test when
/// a file at or below them changes.
const SELECT_ALL_CONFIG: YakconfigKeyRef = YakconfigKeyRef {
    section: "test",
    property: "changed_since_select_all",
};

pub(crate) enum Selection {
    /// Every matched target, for the reason given.
    All(String),
    /// The matched targets that the changes can affect.
    Targets {
        selected: Arc<YakMutSet<TargetLabel>>,
        matched: usize,
    },
}

/// Selects the targets of `pattern` that `changes` can affect.
pub(crate) async fn select(
    ctx: &DiceTransaction,
    changes: &ChangedFiles,
    pattern: &ResolvedPattern<ConfiguredProvidersPatternExtra>,
    target_platform: Option<&TargetLabel>,
) -> yak_error::Result<Selection> {
    if let Some(reason) = select_all_reason(ctx, changes).await? {
        return Ok(Selection::All(reason));
    }
    let cells = ctx.ctx().get_cell_resolver().await?.dupe();
    let changes = Changes::new(changes)?;
    let mut graph = Graph {
        ctx,
        cells,
        changes,
        packages: YakMutMap::default(),
    };
    graph.load_project().await?;
    if let Some(label) = graph.changed_configuration_target() {
        return Ok(Selection::All(format!(
            "the configuration target `{label}` may have changed"
        )));
    }

    let matched = graph.matched_targets(pattern).await?;
    let platforms = graph.platforms(target_platform).await?;
    let mut roots = Vec::new();
    for node in &matched {
        let mut edges: Vec<TargetLabel> = node.tests().map(|t| t.target().dupe()).collect();
        edges.extend(platforms.for_target(*node.label()));
        roots.push((node.label().dupe(), edges));
    }
    let affected = graph.affected(roots).await?;
    let selected: YakMutSet<TargetLabel> = matched
        .iter()
        .filter(|node| {
            affected.contains(node.label())
                || node.tests().any(|test| affected.contains(test.target()))
        })
        .map(|node| node.label().dupe())
        .collect();
    Ok(Selection::Targets {
        selected: Arc::new(selected),
        matched: matched.len(),
    })
}

async fn select_all_reason(
    ctx: &DiceTransaction,
    changes: &ChangedFiles,
) -> yak_error::Result<Option<String>> {
    if let Some(difference) = &changes.config_difference {
        return Ok(Some(format!("the configuration changed at {difference}")));
    }
    if let Some(submodule) = changes.submodules.first() {
        return Ok(Some(format!("the submodule `{submodule}` changed")));
    }
    let root_cell = ctx.ctx().get_cell_resolver().await?.root_cell();
    let configured = ctx
        .ctx()
        .get_legacy_config_property(root_cell, SELECT_ALL_CONFIG)
        .await?;
    let configured = configured.as_deref().unwrap_or_default();
    let select_all: Vec<&str> = SELECT_ALL_FILES
        .iter()
        .copied()
        .chain(
            configured
                .split(',')
                .map(str::trim)
                .filter(|p| !p.is_empty()),
        )
        .map(|p| p.trim_end_matches('/'))
        .collect();
    let changed = changes
        .added
        .iter()
        .chain(&changes.removed)
        .chain(&changes.modified);
    for path in changed {
        let matches = |p: &&str| {
            path == p
                || path
                    .strip_prefix(*p)
                    .is_some_and(|rest| rest.starts_with('/'))
        };
        if select_all.iter().any(matches) {
            return Ok(Some(format!("`{path}` changed")));
        }
    }
    Ok(None)
}

/// `Changes` indexes the changed paths for the lookups that the selection makes.
struct Changes {
    /// Every changed path.
    paths: YakMutSet<ProjectRelativePathBuf>,
    /// Every directory that holds a changed path at any depth, for inputs that are directories.
    dirs: YakMutSet<ProjectRelativePathBuf>,
    /// The paths that appeared or disappeared.
    added_or_removed: Vec<ProjectRelativePathBuf>,
    /// Whether a change can alter the output of `cargo metadata`.
    cargo_metadata: bool,
}

impl Changes {
    fn new(changes: &ChangedFiles) -> yak_error::Result<Self> {
        let parse = |p: &String| -> yak_error::Result<ProjectRelativePathBuf> {
            Ok(ProjectRelativePath::new(p)?.to_owned())
        };
        let added_or_removed = changes
            .added
            .iter()
            .chain(&changes.removed)
            .map(parse)
            .collect::<yak_error::Result<Vec<_>>>()?;
        let mut paths: YakMutSet<ProjectRelativePathBuf> =
            added_or_removed.iter().cloned().collect();
        for path in &changes.modified {
            paths.insert(parse(path)?);
        }
        let mut dirs = YakMutSet::default();
        for path in &paths {
            let mut dir = path.parent();
            while let Some(d) = dir {
                if !dirs.insert(d.to_owned()) {
                    break;
                }
                dir = d.parent();
            }
        }
        let cargo_metadata = changes
            .added
            .iter()
            .chain(&changes.removed)
            .any(|p| yak_external_cells_cargo::is_metadata_input(p, true))
            || changes
                .modified
                .iter()
                .any(|p| yak_external_cells_cargo::is_metadata_input(p, false));
        Ok(Self {
            paths,
            dirs,
            added_or_removed,
            cargo_metadata,
        })
    }
}

/// `Package` is a package's evaluation and whether it can differ from the merge base.
struct Package {
    result: yak_error::Result<Arc<EvaluationResult>>,
    changed: bool,
}

struct Graph<'a> {
    ctx: &'a DiceTransaction,
    cells: CellResolver,
    changes: Changes,
    packages: YakMutMap<PackageLabel, Package>,
}

impl Graph<'_> {
    async fn evaluate(
        &self,
        packages: Vec<PackageLabel>,
    ) -> Vec<(PackageLabel, yak_error::Result<Arc<EvaluationResult>>)> {
        join_all(packages.into_iter().map(|package| {
            let ctx = self.ctx.dupe();
            async move {
                let result = ctx
                    .ctx()
                    .get_interpreter_results(package.dupe())
                    .await
                    .map(|r| r.dupe());
                (package, result)
            }
        }))
        .await
    }

    fn is_external(&self, cell: CellName) -> yak_error::Result<bool> {
        Ok(self.cells.get(cell)?.external().is_some())
    }

    /// Reports whether the files of an external cell can differ from the merge base. A git cell
    /// is pinned in the configuration, and a bundled cell comes with the binary.
    fn external_cell_changed(&self, cell: CellName) -> yak_error::Result<bool> {
        Ok(match self.cells.get(cell)?.external() {
            Some(ExternalCellOrigin::Cargo(_)) => self.changes.cargo_metadata,
            Some(ExternalCellOrigin::Bundled(_) | ExternalCellOrigin::Git(_)) | None => false,
        })
    }

    /// Evaluates every package of the cells in the project and decides which can differ from the
    /// merge base.
    async fn load_project(&mut self) -> yak_error::Result<()> {
        let roots: Vec<CellPath> = self
            .cells
            .cells()
            .filter(|(_, instance)| instance.external().is_none())
            .map(|(name, _)| CellPath::new(name, CellRelativePath::empty().to_owned()))
            .collect();
        let packages: Vec<PackageLabel> = find_package_roots_stream(self.ctx, roots)
            .try_collect()
            .await?;
        let results = self.evaluate(packages).await;

        let mut dirs = YakMutMap::default();
        for (package, _) in &results {
            dirs.insert(
                self.cells.resolve_path(package.as_cell_path())?,
                package.dupe(),
            );
        }
        let owner = |path: &ProjectRelativePath| -> Option<PackageLabel> {
            let mut dir = Some(path);
            while let Some(d) = dir {
                if let Some(package) = dirs.get(d) {
                    return Some(package.dupe());
                }
                dir = d.parent();
            }
            None
        };

        let mut changed: YakMutSet<PackageLabel> = YakMutSet::default();
        // A file that appears or disappears can change the globs of the package that holds it.
        for path in &self.changes.added_or_removed {
            changed.extend(path.parent().and_then(owner));
        }
        let mut build_files: YakMutMap<CellName, Arc<[FileNameBuf]>> = YakMutMap::default();
        let mut package_file_dirs = Vec::new();
        for path in &self.changes.paths {
            let (Some(dir), Some(name)) = (path.parent(), path.file_name()) else {
                continue;
            };
            if PackageFilePath::package_file_names().any(|n| n == name) {
                package_file_dirs.push(dir.to_owned());
                continue;
            }
            let cell = self.cells.find(path);
            if let std::collections::hash_map::Entry::Vacant(entry) = build_files.entry(cell) {
                entry.insert(self.ctx.ctx().get_buildfiles(cell).await?.dupe());
            }
            if build_files[&cell]
                .iter()
                .any(|n| AsRef::<FileName>::as_ref(n) == name)
            {
                // A build file that appears or disappears moves files between its directory's
                // package and the package above it.
                changed.extend(owner(dir));
                changed.extend(dir.parent().and_then(owner));
            }
        }

        let package_imports = self.package_imports(&results).await?;
        let changed_modules = self.changed_modules(&package_imports).await?;

        for (package, result) in results {
            let dir = self.cells.resolve_path(package.as_cell_path())?;
            let reason = if changed.contains(&package) {
                Some("a file in it or its build file changed".to_owned())
            } else if result.is_err() {
                Some("it fails to load".to_owned())
            } else if package_file_dirs.iter().any(|d| dir.starts_with(d)) {
                Some("a `PACKAGE` file above it changed".to_owned())
            } else {
                match package_imports.get(&package) {
                    None => Some("a `PACKAGE` file above it fails to load".to_owned()),
                    Some(imports) => imports
                        .iter()
                        .find(|i| changed_modules.contains(*i))
                        .map(|i| format!("it loads `{i}`, which changed")),
                }
            };
            if let Some(reason) = &reason {
                tracing::debug!("`{package}` may have changed: {reason}");
            }
            let package_changed = reason.is_some();
            self.packages.insert(
                package,
                Package {
                    result,
                    changed: package_changed,
                },
            );
        }
        Ok(())
    }

    /// Returns the files each package loads directly, from its build file and from the `PACKAGE`
    /// files that apply to it. A package whose `PACKAGE` files fail to load has no entry.
    async fn package_imports(
        &self,
        results: &[(PackageLabel, yak_error::Result<Arc<EvaluationResult>>)],
    ) -> yak_error::Result<YakMutMap<PackageLabel, Vec<ImportPath>>> {
        let mut dir_imports: YakMutMap<PackageLabel, Option<Vec<ImportPath>>> =
            YakMutMap::default();
        let mut imports = YakMutMap::default();
        for (package, result) in results {
            let Ok(result) = result else {
                continue;
            };
            let mut package_imports = result.imports().to_vec();
            let mut complete = true;
            let mut dir = Some(package.dupe());
            while let Some(d) = dir {
                if !dir_imports.contains_key(&d) {
                    let deps = INTERPRETER_CALCULATION_IMPL
                        .get()?
                        .get_package_file_deps(&mut self.ctx.ctx(), d.dupe())
                        .await;
                    let deps = deps
                        .ok()
                        .map(|deps| deps.map(|(_, i)| i).unwrap_or_default());
                    dir_imports.insert(d.dupe(), deps);
                }
                match &dir_imports[&d] {
                    Some(i) => package_imports.extend(i.iter().cloned()),
                    None => complete = false,
                }
                // A `PACKAGE` file in an enclosing cell applies too, as in `PACKAGE` evaluation.
                dir = match d.parent()? {
                    Some(parent) => Some(parent),
                    None => match self.cells.resolve_path(d.as_cell_path())?.parent() {
                        None => None,
                        Some(parent) => Some(PackageLabel::from_cell_path(
                            self.cells.get_cell_path(parent).as_ref(),
                        )?),
                    },
                };
            }
            if complete {
                imports.insert(package.dupe(), package_imports);
            }
        }
        Ok(imports)
    }

    /// Returns the loaded files, among those the packages reach, that changed or load a file that
    /// changed. A file that fails to load counts as changed.
    async fn changed_modules(
        &self,
        package_imports: &YakMutMap<PackageLabel, Vec<ImportPath>>,
    ) -> yak_error::Result<YakMutSet<ImportPath>> {
        let mut modules: YakMutMap<ImportPath, Option<Vec<ImportPath>>> = YakMutMap::default();
        let mut frontier: Vec<ImportPath> = package_imports.values().flatten().cloned().collect();
        while !frontier.is_empty() {
            let mut next = YakMutSet::default();
            for path in frontier {
                if !modules.contains_key(&path) {
                    next.insert(path);
                }
            }
            let loaded = join_all(next.into_iter().map(|path| {
                let ctx = self.ctx.dupe();
                async move {
                    let imports = ctx.ctx().get_loaded_module_imports(&path).await.ok();
                    (path, imports)
                }
            }))
            .await;
            frontier = Vec::new();
            for (path, imports) in loaded {
                frontier.extend(imports.iter().flatten().cloned());
                modules.insert(path, imports);
            }
        }

        let mut changed = YakMutSet::default();
        let mut unchanged = YakMutSet::default();
        for path in modules.keys() {
            self.module_changed(path, &modules, &mut changed, &mut unchanged)?;
        }
        Ok(changed)
    }

    fn module_changed(
        &self,
        path: &ImportPath,
        modules: &YakMutMap<ImportPath, Option<Vec<ImportPath>>>,
        changed: &mut YakMutSet<ImportPath>,
        unchanged: &mut YakMutSet<ImportPath>,
    ) -> yak_error::Result<bool> {
        if changed.contains(path) {
            return Ok(true);
        }
        if unchanged.contains(path) {
            return Ok(false);
        }
        let cell = path.path().cell();
        let file_changed = if self.is_external(cell)? {
            self.external_cell_changed(cell)?
        } else {
            self.changes
                .paths
                .contains(&self.cells.resolve_path(path.path().as_ref())?)
        };
        let is_changed = match modules.get(path).and_then(Option::as_ref) {
            None => true,
            Some(_) if file_changed => true,
            Some(imports) => {
                let mut any = false;
                for import in imports {
                    if self.module_changed(import, modules, changed, unchanged)? {
                        any = true;
                        break;
                    }
                }
                any
            }
        };
        if is_changed {
            changed.insert(path.clone());
        } else {
            unchanged.insert(path.clone());
        }
        Ok(is_changed)
    }

    /// Returns a configuration target, such as a constraint or a platform, in a changed package.
    /// Transitions and modifiers refer to configuration targets without depending on them, so a
    /// change to one can affect any target.
    fn changed_configuration_target(&self) -> Option<TargetLabel> {
        self.packages
            .values()
            .filter(|p| p.changed)
            .filter_map(|p| p.result.as_ref().ok())
            .flat_map(|result| result.targets().values())
            .find(|node| TargetNodeRef::to_owned(*node).is_configuration_rule())
            .map(|node| node.label().dupe())
    }

    async fn matched_targets(
        &mut self,
        pattern: &ResolvedPattern<ConfiguredProvidersPatternExtra>,
    ) -> yak_error::Result<Vec<TargetNode>> {
        let packages = pattern
            .specs
            .keys()
            .map(|p| p.package.dupe())
            .collect::<Vec<_>>();
        self.load_packages(packages).await?;
        let mut matched = Vec::new();
        for (package, spec) in &pattern.specs {
            // The test driver reports packages that fail to load and targets that are missing.
            let Ok(result) = &self.packages[&package.package].result else {
                continue;
            };
            match spec {
                PackageSpec::All() => {
                    matched.extend(result.targets().values().map(|node| node.to_owned()));
                }
                PackageSpec::Targets(targets) => matched.extend(
                    targets
                        .iter()
                        .filter_map(|(name, _)| result.get_target(name.as_ref()))
                        .map(|node| node.to_owned()),
                ),
            }
        }
        Ok(matched)
    }

    /// Evaluates the packages not evaluated yet. They belong to external cells, or they are
    /// directories of the project without a build file. Evaluation depends only on the files it
    /// reads, so a package that fails to evaluate fails the same way at the merge base unless
    /// those files changed.
    async fn load_packages(&mut self, packages: Vec<PackageLabel>) -> yak_error::Result<()> {
        let missing: YakMutSet<PackageLabel> = packages
            .into_iter()
            .filter(|p| !self.packages.contains_key(p))
            .collect();
        for (package, result) in self.evaluate(missing.into_iter().collect()).await {
            let changed = if self.is_external(package.cell_name())? {
                self.external_cell_changed(package.cell_name())?
            } else {
                // A build file that disappeared leaves a change in the directory.
                let dir = self.cells.resolve_path(package.as_cell_path())?;
                self.changes.dirs.contains(&dir)
            };
            self.packages.insert(package, Package { result, changed });
        }
        Ok(())
    }

    async fn platforms(
        &self,
        target_platform: Option<&TargetLabel>,
    ) -> yak_error::Result<Platforms> {
        let root_cell = self.cells.root_cell();
        let alias_resolver = self.ctx.ctx().get_cell_alias_resolver(root_cell).await?;
        let config = |key| async move {
            self.ctx
                .ctx()
                .get_legacy_config_property(root_cell, key)
                .await
        };
        let detector = match config(YakconfigKeyRef {
            section: "parser",
            property: "target_platform_detector_spec",
        })
        .await?
        {
            Some(spec) => {
                TargetPlatformDetector::parse_spec(&spec, root_cell, &self.cells, &alias_resolver)?
            }
            None => TargetPlatformDetector::empty(),
        };
        let execution = match config(EXECUTION_PLATFORMS_YAKCONFIG).await? {
            Some(label) => Some(TargetLabel::parse(
                &label,
                root_cell,
                &self.cells,
                &alias_resolver,
            )?),
            None => None,
        };
        Ok(Platforms {
            target: target_platform.cloned(),
            detector,
            execution,
        })
    }

    /// Returns the targets that the roots reach through their dependencies and that are changed
    /// or depend on a changed target. Each root comes with extra edges, to its tests and its
    /// platforms.
    async fn affected(
        &mut self,
        roots: Vec<(TargetLabel, Vec<TargetLabel>)>,
    ) -> yak_error::Result<YakMutSet<TargetLabel>> {
        let mut edges: YakMutMap<TargetLabel, Vec<TargetLabel>> = YakMutMap::default();
        let mut changed = Vec::new();
        let mut extra: YakMutMap<TargetLabel, Vec<TargetLabel>> = roots.into_iter().collect();
        let mut frontier: Vec<TargetLabel> = extra.keys().cloned().collect();
        while !frontier.is_empty() {
            frontier.retain(|label| !edges.contains_key(label));
            frontier.sort();
            frontier.dedup();
            self.load_packages(frontier.iter().map(|l| l.pkg().dupe()).collect())
                .await?;
            let mut next = Vec::new();
            for label in frontier {
                let package = &self.packages[&label.pkg()];
                let node = package
                    .result
                    .as_ref()
                    .ok()
                    .and_then(|r| r.get_target(label.name()));
                let mut deps = extra.remove(&label).unwrap_or_default();
                let reason = match node {
                    // A missing target was missing at the merge base too unless its package
                    // changed.
                    None if package.changed => Some("it is missing, and its package changed"),
                    None => None,
                    Some(node) => {
                        deps.extend(node.deps().cloned());
                        deps.extend(node.get_configuration_deps().map(|d| d.target().dupe()));
                        deps.extend(node.to_owned().get_default_target_platform().cloned());
                        if package.changed {
                            Some("its package may have changed")
                        } else if self.inputs_changed(node.inputs())? {
                            Some("one of its inputs changed")
                        } else {
                            None
                        }
                    }
                };
                if let Some(reason) = reason {
                    tracing::debug!("`{label}` changed: {reason}");
                    changed.push(label.dupe());
                }
                next.extend(deps.iter().cloned());
                edges.insert(label, deps);
            }
            frontier = next;
        }

        let mut dependents: YakMutMap<&TargetLabel, Vec<&TargetLabel>> = YakMutMap::default();
        for (label, deps) in &edges {
            for dep in deps {
                dependents.entry(dep).or_default().push(label);
            }
        }
        let mut affected: YakMutSet<TargetLabel> = changed.iter().cloned().collect();
        let mut todo: Vec<&TargetLabel> = changed.iter().collect();
        while let Some(label) = todo.pop() {
            for dependent in dependents.get(label).into_iter().flatten() {
                if affected.insert((*dependent).dupe()) {
                    todo.push(dependent);
                }
            }
        }
        Ok(affected)
    }

    fn inputs_changed(&self, inputs: impl Iterator<Item = CellPath>) -> yak_error::Result<bool> {
        for input in inputs {
            if self.is_external(input.cell())? {
                continue;
            }
            let path = self.cells.resolve_path(input.as_ref())?;
            if self.changes.paths.contains(&path) || self.changes.dirs.contains(&path) {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

/// `Platforms` finds the platforms that configure a matched target and run its actions.
struct Platforms {
    /// The platform from `--target-platforms`.
    target: Option<TargetLabel>,
    detector: TargetPlatformDetector,
    execution: Option<TargetLabel>,
}

impl Platforms {
    fn for_target(&self, label: TargetLabel) -> Vec<TargetLabel> {
        self.target
            .iter()
            .chain(self.detector.detect(&label))
            .chain(&self.execution)
            .cloned()
            .collect()
    }
}
