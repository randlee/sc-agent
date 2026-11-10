```json
{
  "questions": {
    "addressed": {
      "criteria": {
        "no": "The item does not state the restriction: it is silent about consumer-specific types, only adds or describes types, or adds consumer-specific types.",
        "unclear": "The item contains wording that partly restates the restriction, but it cannot be told whether it excludes consumer-specific types.",
        "yes": "The item explicitly states that sc-agent-types or sc-decision-schema hold no types, fields or constants specific to plan-anchor, judge-jev or other consumer applications."
      },
      "instructions": "`anchor.statement` forbids putting any type, field or constant specific to plan-anchor, judge-jev or another consumer application into sc-agent-types or sc-decision-schema. Does `item.text` explicitly state that restriction, for example by saying the crates hold no consumer-specific or plan-anchor/judge-jev types, or that such types stay in the consumer's own crate? Only an explicit statement of the restriction counts. Merely adding generic types, describing the crates, or being silent is not stating it. Judge presence of the stated restriction only, not whether the item breaks it.",
      "type": "choice"
    },
    "contradicted": {
      "criteria": {
        "no": "Everything the item adds to these crates is generic or mirrors the TypeSafe API, or the item adds nothing to them.",
        "unclear": "The item adds something to these crates and it cannot be told whether it is specific to one consumer application.",
        "yes": "The item places in sc-agent-types or sc-decision-schema a type, field or constant that exists for plan-anchor, judge-jev or another specific consumer application."
      },
      "instructions": "`anchor.statement` forbids any type, field or constant in sc-agent-types or sc-decision-schema that is specific to plan-anchor, judge-jev or another application built on sc-agent. Does `item.text` plan to add or keep such content in either crate? This includes whole types, single fields, constants, mocks and test helpers, and content behind a cargo feature or used only in tests; the anchor grants no exception. A type is not forbidden just because its name sounds like a judging concept: a type that mirrors the TypeSafe API or is useful to every consumer is allowed. Silence about the rule is not a contradiction. Judge conflict only.",
      "type": "choice"
    },
    "relevant": {
      "criteria": {
        "no": "It concerns something else and touches neither crate's contents.",
        "unclear": "It is too vague to tell which crates it touches.",
        "yes": "It plans, adds or changes contents of either crate."
      },
      "instructions": "Does `item.text` plan work on the contents (types, fields, constants) of sc-agent-types or sc-decision-schema, the crates `anchor.statement` restricts? Judge overlap only.",
      "type": "choice"
    }
  }
}
```
