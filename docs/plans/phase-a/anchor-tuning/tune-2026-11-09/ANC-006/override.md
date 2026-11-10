```json
{
  "questions": {
    "addressed": {
      "criteria": {
        "no": "The item gives no matching Python API function and no parity check; it is silent about Python availability or only mentions consistency in general terms.",
        "unclear": "The item mentions Python availability but it cannot be told whether a matching function or parity check is planned.",
        "yes": "The item names a matching Python API function for each decide operation it adds or changes, or states or preserves a check that every decide subcommand has a Python API equivalent."
      },
      "instructions": "The anchor requires that every operation available in the decide CLI is also available in the sc-agent Python API (`anchor.statement`). Does `item.text` explicitly provide for this: for each decide subcommand it adds or changes, does it plan a matching sc-agent Python API function, or does it state or keep a check that every decide subcommand has a Python equivalent? A general remark that the CLI and Python API should be consistent, without a concrete matching function or parity check, does not count. Judge presence only, not conflict.",
      "type": "choice"
    },
    "contradicted": {
      "criteria": {
        "no": "The item states no decide operation that lacks a Python equivalent; it is compliant, silent, Python-only, or unrelated.",
        "unclear": "The item text might mean a decide operation lacks a Python equivalent, but it is ambiguous.",
        "yes": "The item states that a decide operation will have no Python API equivalent, is CLI-only, or loses its Python function."
      },
      "instructions": "The anchor requires that every decide CLI operation is also available in the sc-agent Python API; the Python API may offer more than the CLI but not less. Does `item.text` plan a decide operation that is withheld from, missing from, or explicitly not provided in the Python API, or remove a Python function that a decide subcommand still needs? A Python-only function is not a conflict. Silence about Python is not a conflict. Judge conflict only.",
      "type": "choice"
    },
    "relevant": {
      "criteria": {
        "no": "The item works on another part of the system (for example the HTTP layer, logging internals, or docs) and touches neither a decide subcommand nor an sc-agent Python API function.",
        "unclear": "The item text is too vague to tell whether it touches a decide subcommand or a Python API function.",
        "yes": "The item adds, changes, removes, or depends on a decide subcommand or an sc-agent Python API function."
      },
      "instructions": "The state holds one planning item (`item.title`, `item.text`) and one anchor (`anchor.statement`, governing `anchor.scope`). The anchor concerns which operations the decide CLI and the sc-agent Python API offer. Does the work in `item.text` add, change, remove, or depend on a decide CLI subcommand or a function in the sc-agent Python API? Judge subject overlap only, not compliance.",
      "type": "choice"
    }
  }
}
```
