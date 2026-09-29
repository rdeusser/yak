/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::borrow::Cow;
use std::slice;
use std::time::Instant;

use allocative::Allocative;
use async_trait::async_trait;
use dupe::Dupe;
use pagable::Pagable;
use pagable::pagable_typetag;
use starlark::values::OwnedFrozen;
use starlark::values::UnpackValue;
use starlark::values::Value;
use yak_artifact::artifact::artifact_type::Artifact;
use yak_artifact::artifact::artifact_type::OutputArtifact;
use yak_artifact::artifact::build_artifact::BuildArtifact;
use yak_build_api::actions::Action;
use yak_build_api::actions::ActionExecutionCtx;
use yak_build_api::actions::UnregisteredAction;
use yak_build_api::actions::errors::execute_error::ExecuteError;
use yak_build_api::actions::execute::action_executor::ActionExecutionKind;
use yak_build_api::actions::execute::action_executor::ActionExecutionMetadata;
use yak_build_api::actions::execute::action_executor::ActionOutputs;
use yak_build_api::artifact_groups::ArtifactGroup;
use yak_build_api::interpreter::rule_defs::artifact_tagging::ArtifactTag;
use yak_build_api::interpreter::rule_defs::cmd_args::ArtifactPathMapper;
use yak_build_api::interpreter::rule_defs::cmd_args::CommandLineArtifactVisitor;
use yak_build_api::interpreter::rule_defs::cmd_args::CommandLineBuilder;
use yak_build_api::interpreter::rule_defs::cmd_args::CommandLineSink;
use yak_build_api::interpreter::rule_defs::cmd_args::value_as::ValueAsCommandLineLike;
use yak_build_signals::env::WaitingData;
use yak_common::file_ops::metadata::TrackedFileDigest;
use yak_core::category::CategoryRef;
use yak_core::content_hash::ContentBasedPathHash;
use yak_error::BuckErrorOptionContext;
use yak_execute::artifact::artifact_dyn::ArtifactDyn;
use yak_execute::artifact::fs::ExecutorFs;
use yak_execute::execute::command_executor::ActionExecutionTimingData;
use yak_execute::materialize::materializer::WriteRequest;
use yak_hash::BuckIndexMap;
use yak_hash::BuckIndexSet;
use yak_hash::buck_indexmap;

use crate::actions::impls::run::DepFilesPlaceholderArtifactPathMapper;

#[derive(Debug, yak_error::Error)]
#[yak(tag = Tier0)]
enum WriteActionValidationError {
    #[error("WriteAction received no outputs")]
    NoOutputs,
    #[error("WriteAction received more than one output")]
    TooManyOutputs,
    #[error("Expected command line value, got {0}")]
    ContentsNotCommandLineValue(String),
}

pub(crate) struct CommandLineContentBasedInputVisitor {
    pub(crate) content_based_inputs: BuckIndexSet<ArtifactGroup>,
}

impl CommandLineContentBasedInputVisitor {
    pub(crate) fn new() -> Self {
        Self {
            content_based_inputs: Default::default(),
        }
    }
}

impl<'v> CommandLineArtifactVisitor<'v> for CommandLineContentBasedInputVisitor {
    fn visit_input(&mut self, input: ArtifactGroup, _tags: Vec<&ArtifactTag>) {
        if input.path_resolution_may_require_artifact_value() {
            self.content_based_inputs.insert(input);
        }
    }

    fn visit_declared_output(&mut self, _artifact: OutputArtifact<'v>, _tags: Vec<&ArtifactTag>) {}

    fn visit_frozen_output(&mut self, _artifact: Artifact, _tags: Vec<&ArtifactTag>) {}

    fn visit_declared_artifact(
        &mut self,
        declared_artifact: yak_artifact::artifact::artifact_type::DeclaredArtifact<'v>,
        tags: Vec<&ArtifactTag>,
    ) -> yak_error::Result<()> {
        if declared_artifact.has_content_based_path() {
            let artifact = declared_artifact.ensure_bound()?.into_artifact();
            self.visit_input(ArtifactGroup::Artifact(artifact), tags);
        }

        Ok(())
    }

    fn skip_hidden(&self) -> bool {
        true
    }
}

#[derive(Allocative, Debug, Pagable)]
pub(crate) struct UnregisteredWriteAction {
    pub(crate) is_executable: bool,
    pub(crate) absolute: bool,
    pub(crate) macro_files: Option<Box<[Artifact]>>,
    pub(crate) use_dep_files_placeholder_for_content_based_paths: bool,
}

impl UnregisteredAction for UnregisteredWriteAction {
    fn register(
        self: Box<Self>,
        outputs: BuckIndexSet<BuildArtifact>,
        starlark_data: Option<OwnedFrozen<Value<'static>>>,
        _error_handler: Option<OwnedFrozen<Value<'static>>>,
    ) -> yak_error::Result<Box<dyn Action>> {
        let contents = starlark_data.expect("module data to be present");

        let write_action = WriteAction::new(contents, outputs, *self)?;
        Ok(Box::new(write_action))
    }
}

#[derive(Debug, Allocative, Pagable)]
struct WriteAction {
    contents: OwnedFrozen<Value<'static>>, // StarlarkCmdArgs
    output: BuildArtifact,
    inner: UnregisteredWriteAction,
}

impl WriteAction {
    fn new(
        contents: OwnedFrozen<Value<'static>>,
        outputs: BuckIndexSet<BuildArtifact>,
        inner: UnregisteredWriteAction,
    ) -> yak_error::Result<Self> {
        let mut outputs = outputs.into_iter();

        let output = match (outputs.next(), outputs.next()) {
            (Some(o), None) => o,
            (None, ..) => return Err(WriteActionValidationError::NoOutputs.into()),
            (Some(..), Some(..)) => return Err(WriteActionValidationError::TooManyOutputs.into()),
        };

        let is_command_line = contents.by_ref(|v| -> yak_error::Result<bool> {
            Ok(ValueAsCommandLineLike::unpack_value(*v)?.is_some())
        })?;
        if !is_command_line {
            return Err(WriteActionValidationError::ContentsNotCommandLineValue(
                contents.by_ref(|v| v.to_repr()),
            )
            .into());
        }

        Ok(WriteAction {
            contents,
            output,
            inner,
        })
    }

    fn get_contents(
        &self,
        fs: &ExecutorFs,
        artifact_path_mapping: &dyn ArtifactPathMapper,
    ) -> yak_error::Result<String> {
        let mut cli = Vec::<String>::new();

        self.contents.by_ref(|v| {
            WriteCommandLineOptions {
                absolute: self.inner.absolute,
                macro_files: self.inner.macro_files.as_deref(),
            }
            .render(*v, fs, artifact_path_mapping, &mut cli)
        })?;

        Ok(cli.join("\n"))
    }
}

#[pagable_typetag]
#[async_trait]
impl Action for WriteAction {
    fn kind(&self) -> yak_data::ActionKind {
        yak_data::ActionKind::Write
    }

    fn inputs(&self) -> yak_error::Result<Cow<'_, [ArtifactGroup]>> {
        if self.inner.use_dep_files_placeholder_for_content_based_paths {
            return Ok(Cow::Borrowed(&[]));
        }

        let mut visitor = CommandLineContentBasedInputVisitor::new();
        self.contents.by_ref(|v| -> yak_error::Result<()> {
            ValueAsCommandLineLike::unpack_value_err(*v)?
                .0
                .visit_artifacts(&mut visitor)
        })?;
        let mut content_based_inputs = visitor.content_based_inputs;
        if let Some(macro_files) = &self.inner.macro_files {
            for artifact in macro_files {
                if artifact.path_resolution_requires_artifact_value() {
                    content_based_inputs.insert(ArtifactGroup::Artifact(artifact.dupe()));
                }
            }
        }
        Ok(Cow::Owned(content_based_inputs.into_iter().collect()))
    }

    fn outputs(&self) -> Cow<'_, [BuildArtifact]> {
        Cow::Borrowed(slice::from_ref(&self.output))
    }

    fn first_output(&self) -> &BuildArtifact {
        &self.output
    }

    fn category(&self) -> CategoryRef<'_> {
        CategoryRef::unchecked_new("write")
    }

    fn identifier(&self) -> Option<&str> {
        Some(self.output.get_path().path().as_str())
    }

    fn aquery_attributes(
        &self,
        fs: &ExecutorFs,
        artifact_path_mapping: &dyn ArtifactPathMapper,
    ) -> BuckIndexMap<String, String> {
        // TODO(cjhopman): We should change this api to support returning a Result.
        buck_indexmap! {
            "contents".to_owned() => match self.get_contents(fs, artifact_path_mapping) {
                Ok(v) => v,
                Err(e) => format!("ERROR: constructing contents ({e})")
            },
            "absolute".to_owned() => self.inner.absolute.to_string(),
        }
    }

    async fn execute(
        &self,
        ctx: &mut dyn ActionExecutionCtx,
        waiting_data: WaitingData,
    ) -> Result<(ActionOutputs, ActionExecutionMetadata), ExecuteError> {
        let fs = ctx.fs();

        let mut execution_start = None;

        let value = ctx
            .materializer()
            .declare_write(Box::new(|| {
                execution_start = Some(Instant::now());
                let content = if self.inner.use_dep_files_placeholder_for_content_based_paths {
                    self.get_contents(
                        &ctx.executor_fs(),
                        &DepFilesPlaceholderArtifactPathMapper {},
                    )?
                } else {
                    self.get_contents(&ctx.executor_fs(), &ctx.artifact_path_mapping(None))?
                }
                .into_bytes();
                let path = fs.resolve_build(
                    self.output.get_path(),
                    if self.output.get_path().is_content_based_path() {
                        let digest = TrackedFileDigest::from_content(
                            &content,
                            ctx.digest_config().cas_digest_config(),
                        );
                        Some(ContentBasedPathHash::new(digest.raw_digest().as_bytes())?)
                    } else {
                        None
                    }
                    .as_ref(),
                )?;
                Ok(vec![WriteRequest {
                    path,
                    content,
                    is_executable: self.inner.is_executable,
                    path_kind: self.output.get_path().path_resolution_method(),
                }])
            }))
            .await?
            .into_iter()
            .next()
            .internal_error("Write did not execute")?;

        let wall_time = Instant::now()
            - execution_start.internal_error("Action did not set execution_start")?;

        Ok((
            ActionOutputs::new(buck_indexmap![self.output.get_path().dupe() => value]),
            ActionExecutionMetadata {
                dep_file_db_writes_queued: 0,
                execution_kind: ActionExecutionKind::Simple,
                timing: ActionExecutionTimingData { wall_time },
                input_files_bytes: None,
                waiting_data,
            },
        ))
    }
}

pub(crate) struct WriteCommandLineOptions<'a> {
    pub(crate) absolute: bool,
    pub(crate) macro_files: Option<&'a [Artifact]>,
}

impl WriteCommandLineOptions<'_> {
    pub(crate) fn render(
        &self,
        content: Value<'_>,
        fs: &ExecutorFs<'_>,
        artifact_path_mapping: &dyn ArtifactPathMapper,
        sink: &mut dyn CommandLineSink,
    ) -> yak_error::Result<()> {
        let macro_files = self
            .macro_files
            .map(|files| {
                files
                    .iter()
                    .map(|a| a.resolve_path(fs.fs(), artifact_path_mapping.get(a)))
                    .collect::<yak_error::Result<Vec<_>>>()
            })
            .transpose()?;
        let mut builder = CommandLineBuilder::new_with_options(
            sink,
            artifact_path_mapping,
            fs,
            self.absolute,
            macro_files,
        );
        ValueAsCommandLineLike::unpack_value_err(content)?
            .0
            .add_to_command_line(&mut builder)
    }
}

#[cfg(test)]
mod tests {
    // TODO: This needs proper tests, but right now it's kind of a pain to get the
    //       action framework up and running to test actions
    #[test]
    fn writes_file() {}
}
