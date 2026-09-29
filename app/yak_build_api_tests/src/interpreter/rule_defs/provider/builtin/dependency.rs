/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use indoc::indoc;
use starlark::environment::GlobalsBuilder;
use starlark::eval::Evaluator;
use starlark::starlark_module;
use yak_build_api::interpreter::rule_defs::provider::collection::ProviderCollection;
use yak_build_api::interpreter::rule_defs::provider::dependency::Dependency;
use yak_core::configuration::data::ConfigurationData;
use yak_core::pattern::pattern::ParsedPattern;
use yak_core::pattern::pattern_type::ProvidersPatternExtra;
use yak_interpreter_for_build::interpreter::build_context::BuildContext;
use yak_interpreter_for_build::interpreter::testing::Tester;

#[starlark_module]
fn dependency_creator(builder: &mut GlobalsBuilder) {
    fn create_collection<'v>(
        s: &str,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Dependency<'v>> {
        let c = BuildContext::from_context(eval)?;
        let label = match ParsedPattern::<ProvidersPatternExtra>::parse_precise(
            s,
            c.build_file_cell().name(),
            c.cell_resolver(),
            c.cell_info.cell_alias_resolver(),
        ) {
            Ok(ParsedPattern::Target(package, target_name, providers)) => providers
                .into_providers_label(package, target_name.as_ref())
                .configure(ConfigurationData::testing_new()),
            _ => {
                eprintln!("Expected a target, not {s}");
                panic!();
            }
        };
        let collection =
            eval.frozen_heap(|fh, edge| edge.rebrand(ProviderCollection::testing_new_default(fh)));

        Ok(Dependency::new(eval.heap(), label, collection, None))
    }
}

#[test]
fn dependency_works() -> yak_error::Result<()> {
    let mut tester = Tester::new()?;
    tester.additional_globals(yak_build_api::interpreter::rule_defs::register_rule_defs);
    tester.additional_globals(dependency_creator);
    tester.run_starlark_bzl_test(indoc!(
        r#"
        frozen = create_collection("root//foo:bar[baz]")
        def test():
            notfrozen = create_collection("root//foo:bar[baz]")
            expect = "<dependency root//foo:bar[baz] (<testing>#<HASH>)>"

            assert_eq_ignore_hash(expect, repr(notfrozen))
            assert_eq({}, notfrozen[DefaultInfo].sub_targets)
            assert_eq(["baz"], notfrozen.label.sub_target)

            assert_eq_ignore_hash(expect, repr(frozen))
            assert_eq({}, frozen[DefaultInfo].sub_targets)
            assert_eq(["baz"], frozen.label.sub_target)
        "#
    ))?;
    Ok(())
}
