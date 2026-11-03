# Cross-impl branches fixture

**Producer (a-11):** `sc-treesitter extract --language python --query branches --rev HEAD tests/fixtures/cross-impl/branches/hook.py` emits `subjects.json` from `hook.py` and `queries/python/branches.scm`. The same query ships in `crates/sc-treesitter/assets/queries/python/branches.scm` and is copied here as `queries/python-branches.scm`. This directory is owned only by sprint **a-11**.

| File | Role |
| --- | --- |
| `hook.py` | Source blob read with `git show` |
| `subjects.json` | Expected `sc/subjects/1` stdout, byte-for-byte |
| `request.json` | Expected extract argv and capture names |
| `queries/python-branches.scm` | Query text compiled into the producer |

**Consumer (a-7):** `sc-jev-check` cross-impl tests read the committed artifact and assert payload digests match the Python reference (README tighten **#2**).
