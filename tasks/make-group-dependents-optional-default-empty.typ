#import "@local/mindtape:0.2.0": *

#show: task.with(
  title: "make group dependents optional (default empty)",
  priority: 2,
  difficulty: 2,
  status: done(2026, 7, 2)[commit fix(manifest): default dependents to \[\]; schema regenerated, SKILL.md corrected],
)

= Problem

`Group.dependents` (`src/manifest.rs`) has no `#[serde(default)]`, so a manifest
that omits the `dependents:` key fails to parse:
`missing field 'dependents'` (exit 2). You must write `dependents: []`.
Surfaced while validating the usage skill. A source-only group (no declared
dependents) is a legitimate shape; dependents should default to empty like
`bidirectional` does.

= Change

- `src/manifest.rs`: add `#[serde(default)]` to `pub dependents: Vec<String>`.
- Regenerate the committed schema (`just gen-schema`); `dependents` drops out of
  the required set. This trips this repo's own `schema` group
  (`src/manifest.rs` → `schema/outdatty.schema.json`, `examples/outdatty.yaml`),
  so re-confirm with `outdatty update --group schema` after regenerating.
- Regression test: a manifest with no `dependents:` key loads with an empty
  dependents list.

= Note

The `committed_schema_is_current` test enforces that the schema is regenerated;
skipping `just gen-schema` will fail it.
