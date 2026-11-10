```json
{
  "questions": {
    "addressed": {
      "criteria": {
        "no": "The item does not state that constraint: it is silent about tokio, mentions a runtime choice without the constraint, or adds tokio with default features.",
        "unclear": "The item refers to tokio's features or presence in a way that cannot be read either as stating the constraint or as not stating it.",
        "yes": "The item explicitly states or checks that tokio is absent or used without its default features."
      },
      "instructions": "Does `item.text` itself state the specific constraint in `anchor.statement`, here that the crates involved do not depend on tokio with its default features (for example: no tokio dependency, tokio only with default-features = false, or a check that the dependency tree has no tokio with default features)? Only an explicit statement of that constraint counts. Merely adding other dependencies, or mentioning async runtimes or tokio without stating the constraint, does not count. Judge presence of the constraint only, not whether the item conflicts with it.",
      "type": "choice"
    },
    "contradicted": {
      "criteria": {
        "no": "No stated plan for a named crate does what the anchor forbids, including when the item is silent, undecided, or about another crate.",
        "unclear": "The item states a plan for a named crate whose wording might or might not do what the anchor forbids.",
        "yes": "The item states a plan for a named crate that does what the anchor statement forbids."
      },
      "instructions": "Does `item.text` state a plan or decision that cannot be reconciled with `anchor.statement`, given the crates the statement names? A contradiction is a stated plan that does the forbidden thing to a named crate. Silence about the rule, an undecided option, or work on a crate the statement does not name is not a contradiction. Judge conflict only.",
      "type": "choice"
    },
    "relevant": {
      "criteria": {
        "no": "The item deals with a crate that is not enumerated, or with something other than dependencies or runtime.",
        "unclear": "It is not possible to tell from the text which crate or what kind of change is involved.",
        "yes": "The item deals with dependencies or the execution runtime of an enumerated crate."
      },
      "instructions": "Is `item.text` within the area that `anchor.scope` describes, for one of the crates enumerated in `anchor.statement`? Items that add, change, decide on, or check the dependencies or execution runtime of an enumerated crate are within it. Items about other crates, or about enumerated crates without touching their dependencies or runtime, are not. Judge applicability only.",
      "type": "choice"
    }
  }
}
```
