```json
{
  "questions": {
    "addressed": {
      "criteria": {
        "no": "`item.text` does not state the obligation: it is silent about it, mentions only the topic, or describes a different approach.",
        "unclear": "`item.text` uses wording that might or might not mean the obligation, and it cannot be told which.",
        "yes": "`item.text` explicitly says the thing it adds or changes satisfies the obligation in `anchor.statement` (e.g. it implements the named trait from the named crate)."
      },
      "instructions": "Does `item.text` explicitly state the specific obligation in `anchor.statement` (for this anchor: that the decision backend is an implementation of the named trait in the named crate)? A mention of the topic, of traits in general, or of design work does not count; the item must say the obligation itself.",
      "type": "choice"
    },
    "contradicted": {
      "criteria": {
        "no": "Nothing stated in `item.text` is incompatible with `anchor.statement`, including when the item is silent about it.",
        "unclear": "`item.text` states something that might be incompatible, but it is ambiguous.",
        "yes": "`item.text` states a design, approach, or exception that is incompatible with `anchor.statement`."
      },
      "instructions": "Does `item.text` state a plan or design that cannot be reconciled with `anchor.statement`, for example using a different trait or call path than the one required, or declaring itself an exception the anchor does not grant? Saying nothing about the requirement is silence, not conflict.",
      "type": "choice"
    },
    "relevant": {
      "criteria": {
        "no": "The item concerns a different component or activity that `anchor.scope` and `anchor.statement` do not name.",
        "unclear": "The item text is too vague to tell which component it touches.",
        "yes": "The item adds, changes, reviews, or depends on a thing named in `anchor.scope` or `anchor.statement`."
      },
      "instructions": "The state holds one planning item (`item.title`, `item.text`) and one anchor (`anchor.statement`, with the area it governs in `anchor.scope`). Does the work in `item.text` add, change, review, or depend on something inside `anchor.scope` or the subject of `anchor.statement`? Judge subject overlap only, not obedience.",
      "type": "choice"
    }
  }
}
```
