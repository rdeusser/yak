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

use allocative::Allocative;
use async_trait::async_trait;
use dupe::Dupe;
use gazebo::prelude::*;
use pagable::Pagable;
use pagable::pagable_typetag;
use starlark::values::OwnedFrozen;
use starlark::values::Value;
use yak_artifact::artifact::build_artifact::BuildArtifact;
use yak_build_api::actions::Action;
use yak_build_api::actions::ActionExecutionCtx;
use yak_build_api::actions::UnregisteredAction;
use yak_build_api::actions::box_slice_set::BoxSliceSet;
use yak_build_api::actions::errors::execute_error::ExecuteError;
use yak_build_api::actions::execute::action_executor::ActionExecutionKind;
use yak_build_api::actions::execute::action_executor::ActionExecutionMetadata;
use yak_build_api::actions::execute::action_executor::ActionOutputs;
use yak_build_api::artifact_groups::ArtifactGroup;
use yak_build_signals::env::WaitingData;
use yak_core::category::CategoryRef;
use yak_core::content_hash::ContentBasedPathHash;
use yak_error::YakErrorOptionContext;
use yak_execute::artifact::artifact_dyn::ArtifactDyn;
use yak_execute::artifact_utils::ArtifactValueBuilder;
use yak_execute::execute::command_executor::ActionExecutionTimingData;
use yak_execute::materialize::materializer::CopiedArtifact;
use yak_hash::YakIndexSet;
use yak_hash::yak_indexset;

#[derive(Debug, yak_error::Error)]
#[yak(tag = Input)]
enum CopyActionValidationError {
    #[error("Exactly one output file must be specified for a copy action, got {0}")]
    WrongNumberOfOutputs(usize),
    #[error("Only artifact inputs are supported in copy actions, got {0}")]
    UnsupportedInput(ArtifactGroup),
}

#[derive(Debug, Allocative, Pagable, Clone, Copy, Dupe)]
pub(crate) enum CopyMode {
    Copy {
        // Override the destination executable bit to +x (true) or -x (false)
        executable_bit_override: Option<bool>,
    },
    Symlink,
}

#[derive(Allocative)]
pub(crate) struct UnregisteredCopyAction {
    src: ArtifactGroup,
    copy: CopyMode,
}

impl UnregisteredCopyAction {
    pub(crate) fn new(src: ArtifactGroup, copy: CopyMode) -> Self {
        Self { src, copy }
    }
}

impl UnregisteredAction for UnregisteredCopyAction {
    fn register(
        self: Box<Self>,
        outputs: YakIndexSet<BuildArtifact>,
        _starlark_data: Option<OwnedFrozen<Value<'static>>>,
        _error_handler: Option<OwnedFrozen<Value<'static>>>,
    ) -> yak_error::Result<Box<dyn Action>> {
        Ok(Box::new(CopyAction::new(self.copy, self.src, outputs)?))
    }
}

#[derive(Debug, Allocative, Pagable)]
struct CopyAction {
    copy: CopyMode,
    inputs: BoxSliceSet<ArtifactGroup>,
    outputs: BoxSliceSet<BuildArtifact>,
}

impl CopyAction {
    fn new(
        copy: CopyMode,
        src: ArtifactGroup,
        outputs: YakIndexSet<BuildArtifact>,
    ) -> yak_error::Result<Self> {
        // TODO: Exclude other variants once they become available here. For now, this is a noop.
        match src {
            ArtifactGroup::Artifact(..) | ArtifactGroup::Promise(..) => {}
            ArtifactGroup::TransitiveSetProjection(..) => {
                return Err(CopyActionValidationError::UnsupportedInput(src.dupe()).into());
            }
        };

        if outputs.len() != 1 {
            Err(CopyActionValidationError::WrongNumberOfOutputs(outputs.len()).into())
        } else {
            Ok(CopyAction {
                copy,
                inputs: BoxSliceSet::from(yak_indexset![src]),
                outputs: BoxSliceSet::from(outputs),
            })
        }
    }

    fn input(&self) -> &ArtifactGroup {
        self.inputs
            .iter()
            .next()
            .expect("a single input by construction")
    }

    fn output(&self) -> &BuildArtifact {
        self.outputs
            .iter()
            .next()
            .expect("a single artifact by construction")
    }
}

#[pagable_typetag]
#[async_trait]
impl Action for CopyAction {
    fn kind(&self) -> yak_data::ActionKind {
        yak_data::ActionKind::Copy
    }

    fn inputs(&self) -> yak_error::Result<Cow<'_, [ArtifactGroup]>> {
        Ok(Cow::Borrowed(self.inputs.as_slice()))
    }

    fn outputs(&self) -> Cow<'_, [BuildArtifact]> {
        Cow::Borrowed(self.outputs.as_slice())
    }

    fn first_output(&self) -> &BuildArtifact {
        self.output()
    }

    fn category(&self) -> CategoryRef<'_> {
        CategoryRef::unchecked_new("copy")
    }

    fn identifier(&self) -> Option<&str> {
        Some(self.output().get_path().path().as_str())
    }

    async fn execute(
        &self,
        ctx: &mut dyn ActionExecutionCtx,
        waiting_data: WaitingData,
    ) -> Result<(ActionOutputs, ActionExecutionMetadata), ExecuteError> {
        let (input, src_value) = ctx
            .artifact_values(self.input())
            .iter()
            .into_singleton()
            .internal_error("Input did not dereference to exactly one artifact")?;

        let artifact_fs = ctx.fs();
        let src = input.resolve_path(
            artifact_fs,
            if input.path_resolution_requires_artifact_value() {
                Some(src_value.content_based_path_hash())
            } else {
                None
            }
            .as_ref(),
        )?;
        let tmp_dest = artifact_fs.resolve_build(
            self.output().get_path(),
            Some(&ContentBasedPathHash::for_output_artifact()),
        )?;

        let value = {
            let fs = artifact_fs.fs();
            let mut builder = ArtifactValueBuilder::new(fs, ctx.digest_config());
            match self.copy {
                CopyMode::Copy {
                    executable_bit_override,
                } => {
                    builder.add_copied(
                        src_value,
                        src.as_ref(),
                        tmp_dest.as_ref(),
                        executable_bit_override,
                    )?;
                }
                CopyMode::Symlink => {
                    builder.add_symlinked(src_value, src.clone(), tmp_dest.as_ref())?;
                }
            }

            builder.build(tmp_dest.as_ref())?
        };

        let dest = if self.output().get_path().is_content_based_path() {
            artifact_fs.resolve_build(
                self.output().get_path(),
                Some(&value.content_based_path_hash()),
            )?
        } else {
            tmp_dest
        };

        ctx.materializer()
            .declare_copy(
                dest.clone(),
                value.dupe(),
                // FIXME(JakobDegen): This is wrong in cases where the input artifact is a source
                // directory with ignored paths, as the materializer will incorrectly assume that
                // the source directory matches the artifact value when it doesn't.
                vec![CopiedArtifact::new(
                    src,
                    dest,
                    value.entry().dupe().map_dir(|d| d.as_immutable()),
                    match self.copy {
                        CopyMode::Copy {
                            executable_bit_override,
                        } => executable_bit_override,
                        CopyMode::Symlink => None,
                    },
                )],
            )
            .await?;

        Ok((
            ActionOutputs::from_single(self.output().get_path().dupe(), value),
            ActionExecutionMetadata {
                dep_file_db_writes_queued: 0,
                execution_kind: ActionExecutionKind::Simple,
                timing: ActionExecutionTimingData::default(),
                input_files_bytes: None,
                waiting_data,
            },
        ))
    }
}

#[cfg(test)]
mod tests {
    // TODO: This needs proper tests, but right now it's kind of a pain to get the
    //       action framework up and running to test actions
    #[test]
    fn copies_file() {}
}
