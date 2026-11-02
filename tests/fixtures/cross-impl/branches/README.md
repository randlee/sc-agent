# Cross-impl branches fixture

**Producer (a-11):** `sc-treesitter extract --language python --query branches …` emits
`subjects.json` from hook source + `queries/python/branches.scm` (also shipped under
`sc-treesitter` assets). This directory is owned only by sprint **a-11**.

**Consumer (a-7):** `sc-jev-check` cross-impl tests read the committed artifact and assert
payload digests match the Python reference (README tighten **#2**).
