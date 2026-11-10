Round 3 fix verification: both findings are fixed. I read the plan text only. I did not check the underlying sc-sha behavior.

**RBP-F019 (RBP-007): fixed.**
- `docs/plans/phase-a/sprint-a-2-sc-decision-schema.md:57` now says the copied `unreachable!()` arm is deleted and replaced by `Err(error) => Err(error)`, with "no panic path, no `#[allow]`".
- `of_text` returns `Result<Self, sc_sha::ShaError>` (line 271).
- `question_digests` is fallible (line 277). It maps the error to `DecisionError::Encode`, so `DecisionRequest::new` stays total.
- The acceptance line at 385 requires that `digest.rs` has no unreachable!/panic arm.
- The only remaining "unreachable" text is the comment at line 277, "unreachable-by-contract ShaError". It describes a case that cannot occur and is not a panic path.

**RBP-F020 (RBP-004): fixed.**
- `ReleaseDate` ordering-key doc: `docs/plans/phase-a/sprint-a-1-sc-agent-types.md:224` now reads "An ordering key for 'newest release wins', not a calendar date: it does not validate the calendar...". A-1:162 also adds a general note that a `kind: String` is display-only text.
- A-3 `kind` display-only notes: `docs/plans/phase-a/sprint-a-3-sc-treesitter.md:293`, 298, 312 and 316.
- A-10 `kind` display-only notes: `docs/plans/phase-a/sprint-a-10-clis-python.md:316`, 340, 345 and 354.

No new problems were introduced by these fixes.