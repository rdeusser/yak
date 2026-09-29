/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::future::Future;
use std::sync::Arc;

use dice::CancellationContext;
use dice::DiceComputations;
use dice::EqualityBehavior;
use dice::Key;
use dice::OkPagableValueSerialize;
use dice::ValueSerialize;
use dupe::ResultDupedErrExt;
use gazebo::prelude::SliceExt as _;
use gazebo::prelude::VecExt as _;
use pagable::Pagable;
use pagable::pagable_typetag;
use yak_core::cells::name::CellName;
use yak_fs::paths::file_name::FileNameBuf;

use crate::legacy_configs::dice::HasLegacyConfigs;
use crate::legacy_configs::key::YakconfigKeyRef;
use crate::legacy_configs::view::LegacyYakConfigView;

const DEFAULT_BUILDFILES: &[&str] = &["YAK"];

/// parse_buildfile_name returns the build file names of a cell: the list in `buildfile.name`, or
/// `YAK` when the key is unset, followed by `buildfile.extra_for_test` when it is set.
pub fn parse_buildfile_name(
    mut config: impl LegacyYakConfigView,
) -> yak_error::Result<Vec<FileNameBuf>> {
    let mut base = if let Some(buildfiles_value) =
        config.parse_list::<String>(YakconfigKeyRef {
            section: "buildfile",
            property: "name",
        })? {
        buildfiles_value.into_try_map(FileNameBuf::try_from)?
    } else {
        DEFAULT_BUILDFILES.map(|&n| FileNameBuf::try_from(n.to_owned()).unwrap())
    };

    if let Some(buildfile) = config.parse::<String>(YakconfigKeyRef {
        section: "buildfile",
        property: "extra_for_test",
    })? {
        base.push(FileNameBuf::try_from(buildfile)?);
    }

    Ok(base)
}

pub trait HasBuildfiles<'d> {
    fn get_buildfiles(
        &mut self,
        cell: CellName,
    ) -> impl Future<Output = yak_error::Result<&'d Arc<[FileNameBuf]>>>;
}

#[derive(
    Clone,
    derive_more::Display,
    Debug,
    Hash,
    Eq,
    PartialEq,
    allocative::Allocative,
    Pagable
)]
#[display("BuildfilesKey({})", self.0)]
#[pagable_typetag(dice::DiceKeyDyn)]
struct BuildfilesKey(CellName);

#[async_trait::async_trait]
impl Key for BuildfilesKey {
    type Value = yak_error::Result<Arc<[FileNameBuf]>>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        let config = ctx.get_legacy_config_on_dice(self.0).await?;
        Ok(parse_buildfile_name(config.view(ctx))?.into())
    }

    fn equality_behavior() -> EqualityBehavior<Self::Value> {
        EqualityBehavior::Compare(|x, y| match (x, y) {
            (Ok(x), Ok(y)) => x == y,
            _ => false,
        })
    }

    fn value_serialize() -> impl ValueSerialize<Value = Self::Value> {
        OkPagableValueSerialize::<Self::Value>::new()
    }
}

impl<'d> HasBuildfiles<'d> for DiceComputations<'d> {
    async fn get_buildfiles(
        &mut self,
        cell: CellName,
    ) -> yak_error::Result<&'d Arc<[FileNameBuf]>> {
        self.compute(&BuildfilesKey(cell))
            .await?
            .as_ref()
            .duped_err()
    }
}

#[cfg(test)]
mod tests {
    use gazebo::prelude::SliceExt;
    use indoc::indoc;
    use yak_core::cells::name::CellName;

    use crate::buildfiles::parse_buildfile_name;
    use crate::legacy_configs::cells::YakConfigBasedCells;
    use crate::legacy_configs::configs::testing::TestConfigParserFileOps;

    #[tokio::test]
    async fn test_buildfiles() -> yak_error::Result<()> {
        let mut file_ops = TestConfigParserFileOps::new(&[
            (
                ".yakconfig",
                indoc!(
                    r#"
                            [cells]
                                root = .
                                other = other/
                        "#
                ),
            ),
            (
                "other/.yakconfig",
                indoc!(
                    r#"
                            [cells]
                                other = .
                            [buildfile]
                                name = BUILD,YAK
                                extra_for_test = BUILD.test
                        "#
                ),
            ),
        ])?;

        let cells = YakConfigBasedCells::testing_parse_with_file_ops(&mut file_ops, &[]).await?;

        let config = cells
            .parse_single_cell_with_file_ops(CellName::testing_new("root"), &mut file_ops)
            .await?;
        assert_eq!(
            vec!["YAK"],
            parse_buildfile_name(&config)?.map(|f| f.as_str()),
        );

        let config = cells
            .parse_single_cell_with_file_ops(CellName::testing_new("other"), &mut file_ops)
            .await?;
        assert_eq!(
            vec!["BUILD", "YAK", "BUILD.test"],
            parse_buildfile_name(&config)?.map(|f| f.as_str()),
        );

        Ok(())
    }
}
