# Intent - phase-m

Phase m turns sc-agent into a layered, data-driven library that is open to extension and closed to modification.

- Layered: crates sit in four layers and depend only downward. Behaviour that several crates share is written once in the lowest layer they all reach; what differs between them stays above.
- Data-driven: providers, models and retry rules are values. Defaults are seeded in code, and configuration can add to them or override any of them.
- Extensible without edits: adding a provider or a model is configuration. Adding an API shape is one new crate that implements the existing traits. Adding a tool's tables is SQL handed to the existing database crate.
- Thin surfaces: the CLI and the Python wheel hold no logic of their own.
- Minimal: the phase adds no code beyond what its records require and reuses libraries that already do the job.
- Mechanically enforced: layer boundaries and size limits are checked by sc-lint.

The work is behaviour-preserving moves first, then the changes: configuration-fed providers and models, the session log in SQLite, and retry rules as data.

Approved by Rand Lee, 2026-11-28 08:29: 'The draft intent is great, approved.'

## Anchors

Anchors for this plan live in `anchors.json` (record of truth) and are rendered to `anchors.md`: ANC-001 to ANC-033 (ANC-001 to ANC-020 carried from phase j, ANC-021 onward added from 2026-11-27).
