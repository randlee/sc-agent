# Supersedes

This run started a fresh tuning history (`phase-m/ANC-029/planning`, `events: []` in its request) instead of continuing the earlier chain. The tool records no supersession, so it is stated here.

- Earlier record, kept unchanged: `tune-ANC-029-planning-1`, `tune-ANC-029-planning-2` (no result files) and `tune-ANC-029-planning-3` (not accepted; oracle `8632d3e79d61d2aaa674791ea9daa5de1503b5a70f1d4f02351d018a84df4457`).
- This run: oracle `a17cde4ab727a47128e6ae76c4ddeacca856c6ff12ed314614297a813fcb382a`; sealed set `2270b33e390a6dcfe4855ca77c3ad12d54c06c48754f38b8be2405807b7d9508`.
- What changed: ANC-029 was split (statement to v3, "`--resume` is implemented in `sc-llm`"; the resume-as-LLM-session part became ANC-033), commit abeaf48c. The fresh chain was not disclosed at the time (reported by ai-lead, 2026-11-28).
- Owner approved fresh chains 2026-11-28 08:13 ("1 - a"), after the fact for this run.
