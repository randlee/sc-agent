```json
{
  "questions": {
    "addressed": {
      "criteria": {
        "no": "The item does not say the Jev API types live in sc-decision-schema, or it places them somewhere else.",
        "unclear": "The item mentions sc-decision-schema but it cannot be told whether its Jev API types are defined there.",
        "yes": "The item explicitly places, keeps, or imports its Jev API types in or from sc-decision-schema."
      },
      "instructions": "Does `item.text` state that the Jev API types it adds, changes, or uses are defined in the `sc-decision-schema` crate, as `anchor.statement` requires? Count it when the item puts or keeps those types in sc-decision-schema, or imports them from sc-decision-schema. A generic mention of Jev types, serialization, or consistency without naming sc-decision-schema as their home does not count. Judge presence of this placement only, not conflict.",
      "type": "choice"
    },
    "contradicted": {
      "criteria": {
        "no": "Nothing in the item conflicts with the directive, whether or not the directive is mentioned.",
        "unclear": "The item is ambiguous enough that it might or might not conflict with the directive.",
        "yes": "The item states a plan or design that cannot be reconciled with the directive, including a temporary one."
      },
      "instructions": "Does `item.text` plan something that `anchor.statement` rules out? The statement allows no exception, so a temporary or stop-gap departure still conflicts. Silence about the directive is not a conflict. Judge conflict only.",
      "type": "choice"
    },
    "relevant": {
      "criteria": {
        "no": "The item works on something else, such as logging, internal bookkeeping, or a type that is never sent to or received from any TypeSafe endpoint.",
        "unclear": "The item text is too vague to tell whether any type it touches maps to the TypeSafe Jev API.",
        "yes": "The item adds, changes, defines, or uses a type, struct, or field that is sent to or received from the TypeSafe Jev API."
      },
      "instructions": "The state holds one planning item (`item.title`, `item.text`) and one anchor (`anchor.statement`, with the area it governs in `anchor.scope`). Does the work in `item.text` fall inside `anchor.scope`: does it add, change, define, or use a Rust type that maps to a field of the TypeSafe Jev API (POST /v1/systemone or GET /v1/models)? Judge subject overlap only, not whether the item obeys the anchor.",
      "type": "choice"
    }
  }
}
```
