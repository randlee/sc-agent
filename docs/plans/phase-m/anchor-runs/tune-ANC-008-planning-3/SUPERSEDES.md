# tune-ANC-008-planning-3 supersedes earlier ANC-008 planning tunes

Earlier folders: `tune-ANC-008-planning-1` and `tune-ANC-008-planning-2`. They stay as the earlier record.

Old oracle digest: `f1e862de9f53f42e298c3cee637442dd5cc706a4984590058fd63b689a7a155a` (9 cases)
New oracle digest: `ce6df3dd376490d5e1b2de2b0671edc892a5bcb47ac6ba38f7cded9227fc39c7` (10 cases)

What changed: added the case `existing-struct-param`, a new method that takes the existing
`ProviderTarget` struct in place of separate endpoint, model and timeout arguments
(expected roles: addressed=yes, contradicted=no, relevant=yes; verdict satisfied).
All nine earlier cases are unchanged. Because the oracle's cases changed, the tuning history
`phase-m/ANC-008/planning` starts a fresh chain here (events: []).

owner approved 2026-11-28 08:13
