/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::fmt::Display;
use std::fmt::Formatter;
use std::hash::Hash;
use std::hash::Hasher;
use std::str::FromStr;
use std::sync::Arc;
use std::sync::LazyLock;
use std::time::Duration;

use allocative::Allocative;
use derive_more::Display;
use dupe::Dupe;
use itertools::Itertools;
use pagable::Pagable;
use starlark_map::sorted_map::SortedMap;
use static_interner::Intern;
use static_interner::interner;
use yak_data::NetworkAccess as YakNetworkAccess;
use yak_hash::YakHasher;

#[derive(Debug, Eq, Hash, PartialEq, Clone, Dupe, Allocative, Pagable)]
pub struct LocalExecutorOptions {
    pub use_persistent_workers: bool,
}

impl Default for LocalExecutorOptions {
    fn default() -> Self {
        Self {
            use_persistent_workers: true,
        }
    }
}

#[derive(Debug, Eq, Hash, PartialEq, Clone, Allocative, Pagable)]
pub struct RemoteEnabledExecutorOptions {
    pub executor: RemoteEnabledExecutor,
    pub re_properties: RePlatformFields,
    pub re_use_case: RemoteExecutorUseCase,
    pub re_action_key: Option<String>,
    pub cache_upload_behavior: CacheUploadBehavior,
    pub remote_cache_enabled: bool,
    pub remote_dep_file_cache_enabled: bool,
    pub priority: Option<i32>,
}

#[derive(Clone, Debug, Display, Eq, PartialEq, Hash, Allocative, Pagable)]
struct RemoteExecutorUseCaseData(String);

interner!(
    USE_CASE_INTERNER,
    YakHasher,
    RemoteExecutorUseCaseData,
    String,
    str
);

#[derive(Debug, Eq, PartialEq, Copy, Clone, Dupe, Display, Allocative, Pagable)]
pub struct RemoteExecutorUseCase(Intern<RemoteExecutorUseCaseData>);

impl RemoteExecutorUseCase {
    pub fn new(use_case: String) -> Self {
        Self(USE_CASE_INTERNER.intern(RemoteExecutorUseCaseData(use_case)))
    }

    pub fn as_str(&self) -> &'static str {
        self.0.deref_static()
    }

    /// The "yak-default" use case. This is meant to be used when no use case is configured. It's
    /// not meant to be used for convenience when a use case is not available where it's needed!
    pub fn yak_default() -> Self {
        static USE_CASE: LazyLock<RemoteExecutorUseCase> =
            LazyLock::new(|| RemoteExecutorUseCase::new("yak-default".to_owned()));
        *USE_CASE
    }
}

// The derived PartialEq (which uses pointer equality on the interned data) is still correct.
#[allow(clippy::derived_hash_with_manual_eq)]
impl Hash for RemoteExecutorUseCase {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl FromStr for RemoteExecutorUseCase {
    type Err = yak_error::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(RemoteExecutorUseCase::new(s.to_owned()))
    }
}

#[derive(Debug, Default, Eq, PartialEq, Clone, Hash, Allocative, Pagable)]
pub struct RemoteExecutorOptions {
    pub re_max_input_files_bytes: Option<u64>,
    pub re_max_queue_time: Option<Duration>,
    pub re_resource_units: Option<i64>,
}

/// The actual executor portion of a RemoteEnabled executor. It's possible for a RemoteEnabled
/// executor to wrap a local executor, which is a glorified way of saying "this is a local executor
/// with a RE backend for caching".
#[derive(Display, Debug, Eq, PartialEq, Clone, Hash, Allocative, Pagable)]
pub enum RemoteEnabledExecutor {
    #[display("local")]
    Local(LocalExecutorOptions),
    #[display("remote")]
    Remote(RemoteExecutorOptions),
    #[display("hybrid")]
    Hybrid {
        local: LocalExecutorOptions,
        remote: RemoteExecutorOptions,
        level: HybridExecutionLevel,
    },
}

/// Normalized `remote_execution::Platform`. Also implements `Eq`, `Hash`.
#[derive(Default, Debug, Clone, PartialEq, Eq, Hash, Pagable, Allocative)]
pub struct RePlatformFields {
    pub properties: Arc<SortedMap<String, String>>,
}

#[derive(Debug, Eq, PartialEq, Clone, Hash, Pagable, Allocative)]
#[allow(clippy::large_enum_variant)]
pub enum Executor {
    /// This executor only runs local commands.
    Local(LocalExecutorOptions),

    /// This executor interacts with a RE backend. It may use that to read or write to caches, or
    /// to execute commands.
    RemoteEnabled(RemoteEnabledExecutorOptions),
    /// Can't run any actions
    None,
}

impl Display for Executor {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Local(options) => {
                write!(
                    f,
                    "Local + use persistent workers {}",
                    options.use_persistent_workers
                )
            }
            Self::RemoteEnabled(options) => {
                let cache = match options.remote_cache_enabled {
                    true => "enabled",
                    false => "disabled",
                };
                let dep_file_cache = match options.remote_dep_file_cache_enabled {
                    true => "enabled",
                    false => "disabled",
                };
                write!(
                    f,
                    "RemoteEnabled + executor {} + remote cache {} + cache upload {} + remote dep file cache {}",
                    options.executor, cache, options.cache_upload_behavior, dep_file_cache
                )
            }
            Self::None => write!(f, "None"),
        }
    }
}

#[derive(Debug, Eq, PartialEq, Clone, Copy, Dupe, Hash, Pagable, Allocative)]
pub enum PathSeparatorKind {
    Unix,
    Windows,
}

impl PathSeparatorKind {
    pub fn system_default() -> Self {
        if cfg!(windows) {
            Self::Windows
        } else {
            Self::Unix
        }
    }
}

/// Controls how we implement output_dirs, output_files, output_paths in RE actions.
#[derive(
    Debug, Default, Eq, PartialEq, Clone, Copy, Dupe, Hash, Pagable, Allocative
)]
pub enum OutputPathsBehavior {
    /// Ask for things as either files or directories.
    Strict,
    /// Ask for things as either directories when certain or files AND directories.
    Compatibility,
    /// Ask for things using output_paths.
    #[default]
    OutputPaths,
}

impl FromStr for OutputPathsBehavior {
    type Err = yak_error::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "strict" => Ok(OutputPathsBehavior::Strict),
            "compatibility" => Ok(OutputPathsBehavior::Compatibility),
            "output_paths" => Ok(OutputPathsBehavior::OutputPaths),
            _ => Err(yak_error::yak_error!(
                yak_error::ErrorTag::Input,
                "Invalid OutputPathsBehavior: `{}`",
                s
            )),
        }
    }
}

#[derive(
    Display, Debug, Eq, PartialEq, Clone, Copy, Dupe, Hash, Pagable, Allocative
)]
#[derive(Default)]
pub enum CacheUploadBehavior {
    #[display("enabled")]
    Enabled { max_bytes: Option<u64> },
    #[display("disabled")]
    #[default]
    Disabled,
}

pub const NETWORK_ACCESS_VALUES: &[&str] = &["all", "none", "loopback", "strict", "private"];

#[derive(Debug, Eq, PartialEq, Clone, Copy, Dupe, Hash, Pagable, Allocative)]
pub enum ExecutorNetworkAccess {
    All,
    None,
    Loopback,
    Strict,
    Private,
}

impl From<ExecutorNetworkAccess> for YakNetworkAccess {
    fn from(value: ExecutorNetworkAccess) -> Self {
        match value {
            ExecutorNetworkAccess::All => YakNetworkAccess::All,
            ExecutorNetworkAccess::None => YakNetworkAccess::None,
            ExecutorNetworkAccess::Loopback => YakNetworkAccess::Loopback,
            ExecutorNetworkAccess::Strict => YakNetworkAccess::Strict,
            ExecutorNetworkAccess::Private => YakNetworkAccess::Private,
        }
    }
}

impl From<YakNetworkAccess> for ExecutorNetworkAccess {
    fn from(value: YakNetworkAccess) -> Self {
        match value {
            YakNetworkAccess::All => ExecutorNetworkAccess::All,
            YakNetworkAccess::None => ExecutorNetworkAccess::None,
            YakNetworkAccess::Loopback => ExecutorNetworkAccess::Loopback,
            YakNetworkAccess::Strict => ExecutorNetworkAccess::Strict,
            YakNetworkAccess::Private => ExecutorNetworkAccess::Private,
        }
    }
}

pub fn parse_network_access(s: &str) -> yak_error::Result<YakNetworkAccess> {
    match s {
        "all" => Ok(YakNetworkAccess::All),
        "none" => Ok(YakNetworkAccess::None),
        "loopback" => Ok(YakNetworkAccess::Loopback),
        "strict" => Ok(YakNetworkAccess::Strict),
        "private" => Ok(YakNetworkAccess::Private),
        _ => Err(yak_error::yak_error!(
            yak_error::ErrorTag::Input,
            "Invalid network_access value `{}`, expected one of [{}]",
            s,
            NETWORK_ACCESS_VALUES
                .iter()
                .map(|v| format!("`{v}`"))
                .join(", ")
        )),
    }
}

#[derive(Debug, Eq, PartialEq, Clone, Copy, Dupe, Hash, Pagable, Allocative)]
pub struct CommandGenerationOptions {
    pub path_separator: PathSeparatorKind,
    pub output_paths_behavior: OutputPathsBehavior,
    pub use_bazel_protocol_remote_persistent_workers: bool,
    pub network_access: Option<ExecutorNetworkAccess>,
}

#[derive(Debug, Eq, PartialEq, Hash, Allocative, Clone, Pagable)]
pub struct CommandExecutorConfig {
    pub executor: Executor,
    pub options: CommandGenerationOptions,
}

#[derive(Debug, Eq, PartialEq, Clone, Copy, Dupe, Hash, Pagable, Allocative)]
pub enum HybridExecutionLevel {
    /// Expose both executors but only run it in one preferred executor.
    Limited,
    /// Expose both executors, fallback to the non-preferred executor if execution on the preferred
    /// executor doesn't provide a successful response. By default, we fallback only on errors (i.e.
    /// the infra failed), but not on failures (i.e. the job exited with 1). If
    /// `fallback_on_failure` is set, then we also fallback on failures.
    Fallback { fallback_on_failure: bool },
    /// Race both executors.
    Full {
        fallback_on_failure: bool,
        low_pass_filter: bool,
    },
}

impl CommandExecutorConfig {
    pub fn testing_local() -> Arc<CommandExecutorConfig> {
        Arc::new(CommandExecutorConfig {
            executor: Executor::Local(LocalExecutorOptions::default()),
            options: CommandGenerationOptions {
                path_separator: PathSeparatorKind::system_default(),
                output_paths_behavior: Default::default(),
                use_bazel_protocol_remote_persistent_workers: false,
                network_access: None,
            },
        })
    }

    pub fn re_cache_enabled(&self) -> bool {
        match &self.executor {
            Executor::Local(_) => false,
            Executor::RemoteEnabled(options) => options.remote_cache_enabled,
            Executor::None => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_network_access_parse() {
        assert_eq!(
            parse_network_access("all").expect("all should parse"),
            YakNetworkAccess::All
        );
        assert_eq!(
            parse_network_access("none").expect("none should parse"),
            YakNetworkAccess::None
        );
        assert_eq!(
            parse_network_access("loopback").expect("loopback should parse"),
            YakNetworkAccess::Loopback
        );
        assert_eq!(
            parse_network_access("strict").expect("strict should parse"),
            YakNetworkAccess::Strict
        );
        assert_eq!(
            parse_network_access("private").expect("private should parse"),
            YakNetworkAccess::Private
        );
        assert!(
            parse_network_access("default").is_err(),
            "Omitted network_access should use the default executor behavior"
        );
    }
}
