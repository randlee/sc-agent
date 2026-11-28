# Supersedes

This run supersedes `tune-ANC-026-planning-1`, `tune-ANC-026-planning-2` and `tune-ANC-026-planning-3`.

- Old oracle digest (planning-3): `8ad4622fa4c64d57e91f449ef9eb693fe01656e9e7b0829da0905722420deb6a`
- New oracle digest: `1066f7c48ba34e23196f37751286963653c3f6639ade1359248c9ec3550aed9b`
- What changed:
  - silent-migration label fixed by arch-ai's ruling (dec4, 2026-11-28, option a): relevant yes, addressed no, contradicted no, not_addressed. The case text is unchanged; the label confirms the one frozen in planning-3.
  - "core table" defined as a table owned by sc-agent's own schema (owner "sc-agent"); the authority citations now include it and intent v2. No other case label changed.
  - added probe: not added. The optional tool-own-table probe labelled not_applicable conflicts with ANC-026 v3 scope signal 1 ("CREATE TABLE ... in any crate"), so it was left out pending a scope ruling.
- Fresh chain: owner approved 2026-11-28 08:13

Outcome: sealed bundle `R4-A1-C5`, question set sha256 `a3305c0424b78b704e3aed3b653eece5fb50c129f7f9174ae28ebb77c91e023a`.
