```json
{
  "questions": {
    "addressed": {
      "criteria": {
        "no": "The item is silent on the rule, names only its topic or a vague intent, or plans a different action than the one the statement requires.",
        "unclear": "The item states some of the rule's concrete content, but it cannot be told whether the required action itself is planned.",
        "yes": "The item states the anchor's required action and its limit or default as part of the plan."
      },
      "instructions": "`anchor.statement` gives a concrete rule: an action that must happen, the conditions that trigger it, and any limit or default. Does `item.text` state that rule's action together with its limit or default, in its own words or the statement's, as something the item will do? A mention of the topic, a promise to 'handle' it gracefully, or a goal without the concrete action does not count. Judge whether the rule's content is present only, not whether anything else in the item conflicts with it.",
      "type": "choice"
    },
    "contradicted": {
      "criteria": {
        "no": "No concrete commitment in the item rules out what the statement requires.",
        "unclear": "A commitment in the item might rule out the requirement, but it is too ambiguous to tell.",
        "yes": "At least one concrete commitment in the item rules out what the statement requires for a case it governs."
      },
      "instructions": "List to yourself the concrete commitments `item.text` makes. Does any of them rule out what `anchor.statement` requires, or require something the statement rules out, for a case the statement governs? Omission is not conflict, and commitments about cases outside the statement are not conflicts. Judge conflict only.",
      "type": "choice"
    },
    "relevant": {
      "criteria": {
        "no": "The item is about an unrelated part of the system; a reviewer of this anchor could skip it.",
        "unclear": "The item text does not say enough to tell what part of the system it touches.",
        "yes": "The item builds, changes, or relies on something named in anchor.statement or anchor.scope, so a reviewer of this anchor must read it, even if the item ignores or breaks the anchor."
      },
      "instructions": "The state holds one planning item (`item.title`, `item.text`) and one anchor (`anchor.statement`, `anchor.scope`). Would someone checking that the plan honours `anchor.statement` need to read this item, because the item builds, changes, or relies on the behaviour, component, or queue the anchor is about, as listed in `anchor.scope`? Judge subject overlap only, not compliance.",
      "type": "choice"
    }
  }
}
```
