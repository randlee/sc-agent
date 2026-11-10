```json
{
  "questions": {
    "addressed": {
      "criteria": {
        "no": "The item makes no such commitment, postpones it, makes an exception to it, or plans extra code in the crate.",
        "unclear": "Some wording might be a commitment to minimal crate contents, but it is too vague to decide.",
        "yes": "The item explicitly commits the crate to only the decision-API minimum, with no exception."
      },
      "instructions": "Does `item.text` commit to the directive in `anchor.statement` - that the crate holds only the minimum code needed to send and receive the decision API? Look for an explicit commitment in the item: words such as 'only', 'nothing else', or a stated list of what is excluded. Silence about the limit is no. Promising to look at the crate's size later is no. Stating the limit but making an exception to it is no, because the directive allows nothing beyond the minimum. Judge the presence of this commitment only.",
      "type": "choice"
    },
    "contradicted": {
      "criteria": {
        "no": "Everything the item puts in the crate is decision-API code, or the item puts nothing in the crate.",
        "unclear": "The item adds something to the crate whose relation to sending or receiving the decision API cannot be determined.",
        "yes": "The item places, or explicitly allows, code beyond the decision-API minimum inside the crate."
      },
      "instructions": "`anchor.statement` forbids crates/sc-decision-schema from holding any code beyond the minimum needed to send and receive the decision API. Does `item.text` plan to put such extra code into that crate - for example an HTTP client or transport, retries, a cache, logging, report formatting, or CLI code - or explicitly allow an exception for it? Decision request, response, answer and error types and the decision traits are permitted. Silence about the limit is not a conflict. Judge conflict only.",
      "type": "choice"
    },
    "relevant": {
      "criteria": {
        "no": "The item modifies some other area, or only uses or depends on the scoped area and leaves it unmodified.",
        "unclear": "The item text does not say where its code changes land, so it cannot be told whether the scoped area is modified.",
        "yes": "The item adds, removes, or changes code in the area named by anchor.scope."
      },
      "instructions": "The state holds one planning item (`item.title`, `item.text`) and one anchor with its governed area in `anchor.scope`. Does the work in `item.text` add, remove, or change code inside the area named in `anchor.scope` (here: code in crates/sc-decision-schema)? Only an item that itself modifies that area counts. An item that only uses, imports, or depends on that area while leaving it unmodified does not count. Judge whether the item modifies the scoped area only; do not judge whether it obeys the anchor.",
      "type": "choice"
    }
  }
}
```
