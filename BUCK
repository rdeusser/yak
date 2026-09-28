load(":defs.bzl", "buck2_bundle", "pagable_transition_alias")

# The transition builds buck2 with pagable enabled, whatever the configuration
# of the target that depends on it.
pagable_transition_alias(
    name = "buck2",
    actual = "//app/buck2:buck2-bin",
)

# The client binary next to the daemon binary, the layout that the
# client-only build expects. `buck2.py` runs it.
buck2_bundle(
    name = "buck2_bundle",
    buck2 = "//:buck2",
    buck2_client = "//app/buck2:buck2_client-bin",
    visibility = ["PUBLIC"],
)
