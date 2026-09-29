/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;

use assert_matches::assert_matches;
use dice::DiceTransaction;
use dice::UserComputationData;
use dice::testing::DiceBuilder;
use dupe::Dupe;
use maplit::btreemap;
use sorted_vector_map::sorted_vector_map;
use yak_analysis::analysis::calculation::AnalysisKey;
use yak_artifact::actions::key::ActionIndex;
use yak_artifact::actions::key::ActionKey;
use yak_artifact::artifact::artifact_type::Artifact;
use yak_artifact::artifact::artifact_type::testing::BuildArtifactTestingExt;
use yak_artifact::artifact::build_artifact::BuildArtifact;
use yak_artifact::artifact::source_artifact::SourceArtifact;
use yak_build_api::actions::Action;
use yak_build_api::actions::RegisteredAction;
use yak_build_api::actions::calculation::ActionCalculation;
use yak_build_api::actions::calculation::command_details;
use yak_build_api::actions::execute::dice_data::CommandExecutorResponse;
use yak_build_api::actions::execute::dice_data::HasCommandExecutor;
use yak_build_api::actions::execute::dice_data::SetCommandExecutor;
use yak_build_api::actions::execute::dice_data::SetInvalidationTrackingConfig;
use yak_build_api::actions::execute::dice_data::SetReClient;
use yak_build_api::actions::execute::dice_data::set_fallback_executor_config;
use yak_build_api::actions::impls::run_action_knobs::RunActionKnobs;
use yak_build_api::actions::registry::RecordedActions;
use yak_build_api::analysis::AnalysisResult;
use yak_build_api::analysis::registry::RecordedAnalysisValues;
use yak_build_api::artifact_groups::ArtifactGroup;
use yak_build_api::artifact_groups::calculation::ArtifactGroupCalculation;
use yak_build_api::build::detailed_aggregated_metrics::dice::SetDetailedAggregatedMetricsEventsHolder;
use yak_build_api::build::detailed_aggregated_metrics::dice::SetDetailedAggregatedMetricsHandle;
use yak_build_api::build::detailed_aggregated_metrics::events::DetailedAggregatedMetricsHandle;
use yak_build_api::context::SetBuildContextData;
use yak_build_api::keep_going::HasKeepGoing;
use yak_build_api::spawner::BuckSpawner;
use yak_common::dice::cells::SetCellResolver;
use yak_common::dice::data::testing::SetTestingIoProvider;
use yak_common::external_symlink::ExternalSymlink;
use yak_common::file_ops::metadata::FileMetadata;
use yak_common::file_ops::metadata::TrackedFileDigest;
use yak_common::file_ops::testing::TestFileOps;
use yak_common::http::SetHttpClient;
use yak_common::legacy_configs::configs::LegacyBuckConfig;
use yak_common::legacy_configs::dice::inject_legacy_config_for_test;
use yak_configured::nodes::ConfiguredTargetNodeKey;
use yak_core::category::CategoryRef;
use yak_core::cells::CellResolver;
use yak_core::cells::cell_path::CellPath;
use yak_core::cells::cell_root_path::CellRootPathBuf;
use yak_core::cells::name::CellName;
use yak_core::cells::paths::CellRelativePathBuf;
use yak_core::configuration::compatibility::ResultMaybeCompatible;
use yak_core::configuration::data::ConfigurationData;
use yak_core::deferred::base_deferred_key::BaseDeferredKey;
use yak_core::deferred::key::DeferredHolderKey;
use yak_core::execution_types::execution::ExecutionPlatformResolution;
use yak_core::execution_types::executor_config::CommandExecutorConfig;
use yak_core::execution_types::executor_config::RemoteExecutorUseCase;
use yak_core::fs::artifact_path_resolver::ArtifactFs;
use yak_core::fs::project::ProjectRootTemp;
use yak_core::fs::project_rel_path::ProjectRelativePathBuf;
use yak_core::package::source_path::SourcePath;
use yak_core::target::configured_target_label::ConfiguredTargetLabel;
use yak_core::target::label::label::TargetLabel;
use yak_directory::directory::entry::DirectoryEntry;
use yak_events::dispatch::EventDispatcher;
use yak_events::dispatch::with_dispatcher_async;
use yak_execute::artifact_value::ArtifactValue;
use yak_execute::digest_config::DigestConfig;
use yak_execute::digest_config::SetDigestConfig;
use yak_execute::directory::ActionDirectoryMember;
use yak_execute::execute::action_digest::ActionDigest;
use yak_execute::execute::blocking::SetBlockingExecutor;
use yak_execute::execute::blocking::testing::DummyBlockingExecutor;
use yak_execute::execute::cache_uploader::NoOpCacheUploader;
use yak_execute::execute::kind::CommandExecutionKind;
use yak_execute::execute::output::CommandStdStreams;
use yak_execute::execute::prepared::NoOpCommandOptionalExecutor;
use yak_execute::execute::request::CommandExecutionOutput;
use yak_execute::execute::request::OutputType;
use yak_execute::execute::result::CommandExecutionMetadata;
use yak_execute::execute::result::CommandExecutionReport;
use yak_execute::execute::result::CommandExecutionStatus;
use yak_execute::execute::testing_dry_run::DryRunEntry;
use yak_execute::execute::testing_dry_run::DryRunExecutor;
use yak_execute::materialize::materializer::SetMaterializer;
use yak_execute::re::invocation_re_settings::InvocationReSettings;
use yak_execute::re::invocation_re_settings::SetInvocationReSettings;
use yak_execute::re::manager::UnconfiguredRemoteExecutionClient;
use yak_execute_impl::materializers::deferred::NoDiskDeferredMaterializer;
use yak_file_watcher::dep_files::SetDepFileCache;
use yak_file_watcher::dep_files::create_dep_file_cache;
use yak_file_watcher::mergebase::SetMergebase;
use yak_fs::paths::forward_rel_path::ForwardRelativePathBuf;
use yak_hash::StdBuckHashMap;
use yak_hash::buck_indexset;
use yak_http::HttpClientBuilder;
use yak_node::nodes::configured::ConfiguredTargetNode;
use yak_util::time_span::TimeSpan;

use crate::actions::testings::SimpleAction;

fn create_test_configured_target_label() -> ConfiguredTargetLabel {
    TargetLabel::testing_parse("cell//pkg:foo").configure(ConfigurationData::testing_new())
}

fn create_test_build_artifact() -> BuildArtifact {
    let configured_target_label = create_test_configured_target_label();
    let deferred_id = ActionIndex::new(0);
    BuildArtifact::testing_new(configured_target_label, "bar.out", deferred_id)
}

fn create_test_source_artifact(package_label: &str, target_name: &str) -> SourceArtifact {
    SourceArtifact::new(SourcePath::testing_new(package_label, target_name))
}

fn registered_action(
    build_artifact: BuildArtifact,
    action: Box<dyn Action>,
) -> Arc<RegisteredAction> {
    let registered_action = RegisteredAction::new(
        build_artifact.key().dupe(),
        action,
        CommandExecutorConfig::testing_local(),
    );
    Arc::new(registered_action)
}

fn mock_analysis_for_action_resolution(
    mut dice_builder: DiceBuilder,
    action_key: &ActionKey,
    registered_action_arc: Arc<RegisteredAction>,
) -> DiceBuilder {
    let configured_target_label = create_test_configured_target_label();
    let configured_node_key = ConfiguredTargetNodeKey(configured_target_label.dupe());

    assert_eq!(
        &DeferredHolderKey::Base(BaseDeferredKey::TargetLabel(configured_target_label.dupe())),
        action_key.holder_key()
    );

    let mut actions = RecordedActions::new(1);
    actions.insert(action_key.dupe(), registered_action_arc);

    dice_builder = dice_builder.mock_and_return(
        AnalysisKey(configured_target_label.dupe()),
        ResultMaybeCompatible::Compatible(AnalysisResult::new(
            RecordedAnalysisValues::testing_new(
                action_key.holder_key().dupe(),
                Vec::new(),
                actions,
            ),
            None,
            StdBuckHashMap::default(),
            0,
            0,
            None,
        )),
    );

    dice_builder.mock_and_return(
        configured_node_key,
        ResultMaybeCompatible::Compatible(ConfiguredTargetNode::testing_new(
            configured_target_label,
            "foo_lib",
            ExecutionPlatformResolution::new_for_testing(None, Vec::new()),
            vec![],
            None,
        )),
    )
}

async fn make_default_dice_state(
    dry_run_tracker: Arc<Mutex<Vec<DryRunEntry>>>,
    temp_fs: &ProjectRootTemp,
    mocks: Vec<Box<dyn FnOnce(DiceBuilder) -> DiceBuilder>>,
) -> yak_error::Result<DiceTransaction> {
    let fs = temp_fs.path().dupe();

    let cell_resolver = CellResolver::testing_with_name_and_path(
        CellName::testing_new("cell"),
        CellRootPathBuf::new(ProjectRelativePathBuf::unchecked_new("cell-path".into())),
    );
    let output_path = ProjectRelativePathBuf::unchecked_new("yak-out/v2".into());

    let mut dice_builder = DiceBuilder::new();
    dice_builder = dice_builder.set_data(|data| {
        data.set_testing_io_provider(temp_fs);
        data.set_digest_config(DigestConfig::testing_default());
        data.set_invalidation_tracking_config(true);
        data.set_detailed_aggregated_metrics_handle(DetailedAggregatedMetricsHandle::new());
    });

    for mock in mocks.into_iter() {
        dice_builder = mock(dice_builder);
    }

    let mut extra = UserComputationData::new();
    extra.set_keep_going(true);
    struct CommandExecutorProvider {
        dry_run_tracker: Arc<Mutex<Vec<DryRunEntry>>>,
    }
    impl HasCommandExecutor for CommandExecutorProvider {
        fn get_command_executor(
            &self,
            artifact_fs: &ArtifactFs,
            _config: &CommandExecutorConfig,
        ) -> yak_error::Result<CommandExecutorResponse> {
            let executor = Arc::new(DryRunExecutor::new(
                self.dry_run_tracker.dupe(),
                artifact_fs.dupe(),
            ));
            Ok(CommandExecutorResponse {
                executor,
                action_cache_checker: Arc::new(NoOpCommandOptionalExecutor {}),
                remote_dep_file_cache_checker: Arc::new(NoOpCommandOptionalExecutor {}),
                platform: Default::default(),
                cache_uploader: Arc::new(NoOpCacheUploader {}),
                output_trees_download_config:
                    yak_execute::re::output_trees_download_config::OutputTreesDownloadConfig::new(
                        None, true,
                    ),
            })
        }
    }

    set_fallback_executor_config(&mut extra.data, CommandExecutorConfig::testing_local());
    extra.set_command_executor(Box::new(CommandExecutorProvider { dry_run_tracker }));
    extra.set_detailed_aggregated_metrics_events_holder();
    extra.set_blocking_executor(Arc::new(DummyBlockingExecutor { fs: fs.dupe() }));
    extra.set_materializer(Arc::new(NoDiskDeferredMaterializer::testing_new_no_disk(
        fs,
    )?));
    extra.set_re_client(UnconfiguredRemoteExecutionClient::testing_new_dummy());
    extra.set_invocation_re_settings(InvocationReSettings {
        use_case: RemoteExecutorUseCase::buck2_default(),
        cas_configured: false,
    });
    extra.set_http_client(HttpClientBuilder::https_with_system_roots().await?.build());
    extra.set_dep_file_cache(create_dep_file_cache());
    extra.set_mergebase(Default::default());
    extra.data.set(EventDispatcher::null());
    extra.data.set(RunActionKnobs::default());
    extra.spawner = Arc::new(BuckSpawner::current_runtime().unwrap());

    let mut computations = dice_builder.build(extra).unwrap();
    inject_legacy_config_for_test(
        &mut computations,
        CellName::testing_new("root"),
        LegacyBuckConfig::empty(),
    )?;
    computations.set_buck_out_path(Some(output_path))?;
    computations.set_cell_resolver(cell_resolver)?;

    Ok(computations.commit().await)
}

#[tokio::test]
async fn test_get_action_for_artifact() -> yak_error::Result<()> {
    yak_certs::certs::maybe_setup_cryptography();
    let build_artifact = create_test_build_artifact();
    let registered_action = registered_action(
        build_artifact.dupe(),
        Box::new(SimpleAction::new(
            buck_indexset![],
            buck_indexset![build_artifact.dupe()],
            vec![],
            CategoryRef::new("fake_action").unwrap().to_owned(),
            None,
        )),
    );

    let mut dice_builder = DiceBuilder::new();
    dice_builder = mock_analysis_for_action_resolution(
        dice_builder,
        build_artifact.key(),
        registered_action.dupe(),
    );
    let dice_computations = dice_builder
        .build(UserComputationData::new())
        .unwrap()
        .commit()
        .await;

    let result = with_dispatcher_async(
        EventDispatcher::null(),
        ActionCalculation::get_action(&mut dice_computations.ctx(), build_artifact.key()),
    )
    .await;
    assert_eq!(result?, registered_action);
    Ok(())
}

#[tokio::test]
async fn test_build_action() -> yak_error::Result<()> {
    yak_certs::certs::maybe_setup_cryptography();
    let temp_fs = ProjectRootTemp::new()?;
    let build_artifact = create_test_build_artifact();
    let registered_action = registered_action(
        build_artifact.dupe(),
        Box::new(SimpleAction::new(
            buck_indexset![],
            buck_indexset![build_artifact.dupe()],
            vec!["foo".to_owned(), "cmd".to_owned()],
            CategoryRef::new("fake_action").unwrap().to_owned(),
            None,
        )),
    );

    let dry_run_tracker = Arc::new(Mutex::new(vec![]));
    let dice_computations = make_default_dice_state(
        dry_run_tracker.dupe(),
        &temp_fs,
        vec![{
            let action = registered_action.dupe();
            let action_key = build_artifact.key().dupe();
            Box::new(move |builder| {
                mock_analysis_for_action_resolution(builder, &action_key, action)
            })
        }],
    )
    .await?;

    let result =
        ActionCalculation::build_action(&mut dice_computations.ctx(), registered_action.key())
            .await;

    result.unwrap();

    assert_eq!(
        dry_run_tracker.lock().unwrap()[0],
        DryRunEntry {
            args: vec!["foo".to_owned(), "cmd".to_owned()],
            outputs: vec![CommandExecutionOutput::BuildArtifact {
                path: build_artifact.get_path().dupe(),
                output_type: OutputType::File,
            }],
            env: sorted_vector_map![]
        }
    );

    Ok(())
}

#[tokio::test]
async fn test_build_artifact() -> yak_error::Result<()> {
    yak_certs::certs::maybe_setup_cryptography();
    let temp_fs = ProjectRootTemp::new()?;
    let build_artifact = create_test_build_artifact();
    let registered_action = registered_action(
        build_artifact.dupe(),
        Box::new(SimpleAction::new(
            buck_indexset![],
            buck_indexset![build_artifact.dupe()],
            vec!["bar".to_owned(), "cmd".to_owned()],
            CategoryRef::new("fake_action").unwrap().to_owned(),
            None,
        )),
    );

    let dry_run_tracker = Arc::new(Mutex::new(vec![]));
    let dice_computations = make_default_dice_state(dry_run_tracker.dupe(), &temp_fs, {
        let registered_action = registered_action.dupe();
        let action_key = build_artifact.key().dupe();
        vec![Box::new(move |builder| {
            mock_analysis_for_action_resolution(builder, &action_key, registered_action)
        })]
    })
    .await?;

    let result = with_dispatcher_async(
        EventDispatcher::null(),
        ActionCalculation::build_artifact(&mut dice_computations.ctx(), &build_artifact),
    )
    .await;

    result.unwrap();

    assert_eq!(
        dry_run_tracker.lock().unwrap()[0],
        DryRunEntry {
            args: vec!["bar".to_owned(), "cmd".to_owned()],
            outputs: vec![CommandExecutionOutput::BuildArtifact {
                path: build_artifact.get_path().dupe(),
                output_type: OutputType::File,
            }],
            env: sorted_vector_map![]
        }
    );
    Ok(())
}

#[tokio::test]
async fn test_ensure_artifact_build_artifact() -> yak_error::Result<()> {
    yak_certs::certs::maybe_setup_cryptography();
    let temp_fs = ProjectRootTemp::new()?;
    let build_artifact = create_test_build_artifact();
    let registered_action = registered_action(
        build_artifact.dupe(),
        Box::new(SimpleAction::new(
            buck_indexset![],
            buck_indexset![build_artifact.dupe()],
            vec!["ensure".to_owned(), "cmd".to_owned()],
            CategoryRef::new("fake_action").unwrap().to_owned(),
            None,
        )),
    );

    let dry_run_tracker = Arc::new(Mutex::new(vec![]));
    let dice_computations = make_default_dice_state(dry_run_tracker.dupe(), &temp_fs, {
        let registered_action = registered_action.dupe();
        let action_key = build_artifact.key().dupe();
        vec![Box::new(move |builder| {
            mock_analysis_for_action_resolution(builder, &action_key, registered_action)
        })]
    })
    .await?;

    let result = with_dispatcher_async(
        EventDispatcher::null(),
        dice_computations
            .ctx()
            .ensure_artifact_group(&ArtifactGroup::Artifact(build_artifact.dupe().into())),
    )
    .await;

    result.unwrap();

    assert_eq!(
        dry_run_tracker.lock().unwrap()[0],
        DryRunEntry {
            args: vec!["ensure".to_owned(), "cmd".to_owned()],
            outputs: vec![CommandExecutionOutput::BuildArtifact {
                path: build_artifact.get_path().dupe(),
                output_type: OutputType::File,
            }],
            env: sorted_vector_map![]
        }
    );

    Ok(())
}

#[tokio::test]
async fn test_ensure_artifact_source_artifact() -> yak_error::Result<()> {
    yak_certs::certs::maybe_setup_cryptography();
    let digest_config = DigestConfig::testing_default();

    let path = CellPath::new(
        CellName::testing_new("cell"),
        CellRelativePathBuf::unchecked_new("pkg/src.cpp".to_owned()),
    );
    let source_artifact = create_test_source_artifact("cell//pkg", "src.cpp");
    let metadata = FileMetadata {
        digest: TrackedFileDigest::from_content(b"content", digest_config.cas_digest_config()),
        is_executable: true,
    };

    let dice_builder = DiceBuilder::new().set_data(|data| {
        data.set_digest_config(DigestConfig::testing_default());
    });
    let file_ops = TestFileOps::new_with_files_metadata(btreemap![path => metadata.dupe()]);
    let dice_computations = file_ops
        .mock_in_cell(CellName::testing_new("cell"), dice_builder)
        .build(UserComputationData::new())
        .unwrap()
        .commit()
        .await;

    let source_artifact = Artifact::from(source_artifact);
    let input = ArtifactGroup::Artifact(source_artifact.dupe());
    let result = with_dispatcher_async(
        EventDispatcher::null(),
        dice_computations.ctx().ensure_artifact_group(&input),
    )
    .await?
    .iter()
    .cloned()
    .collect::<Vec<_>>();

    assert_eq!(
        &result,
        &[(
            source_artifact,
            ArtifactValue::file(FileMetadata {
                digest: metadata.digest,
                is_executable: metadata.is_executable,
            })
        )],
    );
    Ok(())
}

#[tokio::test]
async fn test_ensure_artifact_external_symlink() -> yak_error::Result<()> {
    yak_certs::certs::maybe_setup_cryptography();
    let path = CellPath::new(
        CellName::testing_new("cell"),
        CellRelativePathBuf::unchecked_new("proj/to_vendor/include".to_owned()),
    );
    let source_artifact = create_test_source_artifact("cell//proj/to_vendor", "include");
    let symlink = Arc::new(
        ExternalSymlink::new(
            PathBuf::from("/opt/vendor"),
            ForwardRelativePathBuf::new("include".to_owned()).unwrap(),
        )
        .unwrap(),
    );

    let dice_builder = DiceBuilder::new().set_data(|data| {
        data.set_digest_config(DigestConfig::testing_default());
    });
    let file_ops = TestFileOps::new_with_symlinks(btreemap![path => symlink.dupe()]);
    let dice_computations = file_ops
        .mock_in_cell(CellName::testing_new("cell"), dice_builder)
        .build(UserComputationData::new())
        .unwrap()
        .commit()
        .await;

    let source_artifact = Artifact::from(source_artifact);
    let input = ArtifactGroup::Artifact(source_artifact.dupe());
    let result = with_dispatcher_async(
        EventDispatcher::null(),
        dice_computations.ctx().ensure_artifact_group(&input),
    )
    .await?
    .iter()
    .cloned()
    .collect::<Vec<_>>();

    assert_eq!(
        &result,
        &[(
            source_artifact,
            ArtifactValue::new(
                DirectoryEntry::Leaf(ActionDirectoryMember::ExternalSymlink(symlink)),
                None
            )
        )]
    );
    Ok(())
}

#[tokio::test]
async fn test_command_details_omission() {
    use yak_data::command_execution_kind::Command;

    yak_certs::certs::maybe_setup_cryptography();
    let digest_config = DigestConfig::testing_default();

    let mut report = CommandExecutionReport {
        claim: None,
        status: CommandExecutionStatus::Success {
            execution_kind: CommandExecutionKind::Local {
                digest: ActionDigest::empty(digest_config.cas_digest_config()),
                command: vec![],
                env: sorted_vector_map![],
            },
        },
        timing: CommandExecutionMetadata::empty(TimeSpan::empty_now()),
        std_streams: CommandStdStreams::Local {
            stdout: "stdout".to_owned().into_bytes(),
            stderr: "stderr".to_owned().into_bytes(),
        },
        exit_code: Some(1),
        additional_message: None,
    };

    let proto = command_details(&report, false).await;
    let command_kind = proto.command_kind.unwrap();
    assert_matches!(command_kind.command, Some(Command::LocalCommand(..)));
    assert_eq!(&proto.cmd_stdout, "stdout");
    assert_eq!(&proto.cmd_stderr, "stderr");

    let proto = command_details(&report, true).await;
    let command_kind = proto.command_kind.unwrap();
    assert_matches!(command_kind.command, Some(Command::OmittedLocalCommand(..)));
    assert_eq!(&proto.cmd_stdout, "");
    assert_eq!(&proto.cmd_stderr, "stderr");

    report.status = CommandExecutionStatus::Failure {
        execution_kind: CommandExecutionKind::Local {
            digest: ActionDigest::empty(digest_config.cas_digest_config()),
            command: vec![],
            env: sorted_vector_map![],
        },
    };
    let proto = command_details(&report, true).await;
    let command_kind = proto.command_kind.unwrap();
    assert_matches!(command_kind.command, Some(Command::LocalCommand(..)));
    assert_eq!(&proto.cmd_stdout, "stdout");
    assert_eq!(&proto.cmd_stderr, "stderr");
}
