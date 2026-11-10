```json
{
  "questions": {
    "addressed": {
      "criteria": {
        "no": "The item does not say lower-layer errors are kept: it is silent about them, mentions error handling only in general terms, or discards them.",
        "unclear": "The item mentions keeping some error information, but it is not possible to tell whether the lower-layer error's id and message are kept.",
        "yes": "The item states that each error caused by a lower-layer error keeps that error, or its id and message, by wrapping it, holding it as a source or field, or asserting it in a test."
      },
      "instructions": "The anchor requires that every error returned by a public sc-agent function must carry the id and message of each lower-layer error that caused it. Does `item.text` explicitly plan for its errors to carry the id and message of the lower-layer error that caused them, for example by wrapping or storing the source error, exposing it through the error chain, or asserting it in a test? An error that has no lower-layer cause has nothing to carry. A general promise of better or clearer error handling, without saying the lower-layer error is kept, does not count. Judge whether the obligation is present only, not whether the item conflicts with it.",
      "type": "choice"
    },
    "contradicted": {
      "criteria": {
        "no": "The item states no approach that loses a lower-layer error's id or message; silence counts as no.",
        "unclear": "The item's wording about lower-layer errors is ambiguous between keeping and dropping them.",
        "yes": "The item states an approach that returns a caused error without the lower-layer error's id or message."
      },
      "instructions": "Does `item.text` rule out or undo the anchor's requirement that every error returned by a public sc-agent function must carry the id and message of each lower-layer error that caused it? Conflict means the plan states an approach under which a caused error is returned without the lower-layer error's id or message (for example a fixed generic message, an opaque string summary, or discarding the cause). Not mentioning the requirement is silence, not conflict. Judge conflict only.",
      "type": "choice"
    },
    "relevant": {
      "criteria": {
        "no": "The item works on something else (docs, build, private internals) and alters no error type and no public return type.",
        "unclear": "The item is too vague to tell whether it touches an error type or a public return type.",
        "yes": "The item works on an error type or on a public function's return type, directly or as a clear prerequisite."
      },
      "instructions": "`anchor.statement` is about errors: every error returned by a public sc-agent function must carry the id and message of each lower-layer error that caused it. Does `item.text` plan work that defines, adds, changes, or wraps an error type, or changes the return type of a public sc-agent function? Overlap of subject only; obedience is judged elsewhere.",
      "type": "choice"
    }
  }
}
```
