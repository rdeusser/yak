/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use dupe::Dupe;
use yak_core::cells::name::CellName;
use yak_core::cells::paths::CellRelativePath;
use yak_core::package::PackageLabel;
use yak_fs::paths::forward_rel_path::ForwardRelativePath;
use yak_node::attrs::attr_type::list::ListLiteral;
use yak_node::attrs::attr_type::string::StringLiteral;
use yak_node::attrs::coerced_attr::CoercedAttr;
use yak_node::attrs::hacks;
use yak_util::arc_str::ArcSlice;
use yak_util::arc_str::ArcStr;

#[test]
fn stringifies_correctly() -> yak_error::Result<()> {
    let coerced = CoercedAttr::String(StringLiteral(ArcStr::from("Hello, world!")));

    let package = PackageLabel::new(
        CellName::testing_new("root"),
        CellRelativePath::new(ForwardRelativePath::new("foo/bar").unwrap()),
    )?;

    assert_eq!(
        "Hello, world!".to_owned(),
        hacks::value_to_string(&coerced, package.dupe())?
    );

    let list = CoercedAttr::List(ListLiteral(ArcSlice::new([CoercedAttr::String(
        StringLiteral(ArcStr::from("Hello, world!")),
    )])));
    assert!(hacks::value_to_string(&list, package.dupe()).is_err());
    Ok(())
}
