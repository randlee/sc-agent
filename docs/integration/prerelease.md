# Owner-run prerelease checks

Install the intended prerelease with `/prerelease --install` and activate its
`venv`. Run the existing wheel tests from the checkout against that installed
wheel, with the owner present:

```sh
SC_AGENT_LIVE=1 python -m pytest crates/sc-agent-py/tests/test_api.py -m live_outcome -v
```

This explicitly uses the owner's real configuration and credentials. It checks
one default TypeSafe models list and noul `send`/`send_batch` outcomes for `jev`,
`haiku` and `luna`. The wheel selects each model's actual provider; no provider is
forced. A native missing-key error skips that model. Any other error fails.

Without `SC_AGENT_LIVE=1`, the same outcomes use the loopback runner. All
nonmarked tests remain loopback-only, including request inspection, fixed-answer
and local-log assertions. The platform-keyring trap remains armed for them.

These are hand-run product checks. No integration evidence, report or publication
gate is produced. Actual credentialed runs require lead coordination and owner
attendance; agents must not run this live command unattended.
