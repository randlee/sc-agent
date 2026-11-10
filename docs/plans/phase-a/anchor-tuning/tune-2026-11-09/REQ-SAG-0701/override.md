```json
{
  "questions": {
    "addressed": {
      "criteria": {
        "no": "The item does not state parallel dispatch across a configurable worker count: it is silent about how requests are dispatched, mentions only speed, performance or concurrency in general terms, or describes serial or single-worker dispatch.",
        "unclear": "The item mentions parallel dispatch of a batch's requests, but it cannot be told whether the worker count is configurable or what its default is.",
        "yes": "The item states parallel or concurrent dispatch of a batch's requests across a configurable number of workers or threads with default 8, in any wording."
      },
      "instructions": "`anchor.statement` requires that the requests of one batch are sent in parallel across a configurable number of worker threads, 8 by default. Does `item.text` state this: that a batch's requests are sent in parallel (concurrently) across multiple workers or threads whose number is configurable, with a default of 8? Any wording counts (a `workers` setting, a pool size, a concurrency limit). Judge only whether this content is present in the item; do not judge whether something else in the item conflicts with it.",
      "type": "choice"
    },
    "contradicted": {
      "criteria": {
        "no": "Nothing in the item conflicts with the directive, including items that are silent about dispatch, only mention the topic, comply with it, or are about something else.",
        "unclear": "The item describes batch dispatch in a way that might or might not rule out parallel sending across a configurable worker count with default 8.",
        "yes": "The item states a batch-dispatch approach that rules out parallel sending across a configurable worker count with default 8, always or under some condition."
      },
      "instructions": "`anchor.statement` requires that the requests of one batch are sent in parallel across a configurable number of worker threads, 8 by default. Does `item.text` plan batch handling that cannot be reconciled with this? Conflicts include sending a batch's requests one after another or on the caller's thread, limiting batches to a single worker, a fixed non-configurable worker count, a different default, or a condition under which batches run serially that the anchor does not allow. A caller choosing a particular worker count through the configurable setting is not a conflict. Silence about dispatch, or a vague mention of performance, is not a conflict. Judge conflict only.",
      "type": "choice"
    },
    "relevant": {
      "criteria": {
        "no": "The item is about another part of the system, such as the HTTP layer, schemas, secrets or parsing, and does not touch batch handling in sc-decision-runner or decide.",
        "unclear": "The item text is too vague to tell whether it touches batch handling in sc-decision-runner or decide.",
        "yes": "The item plans, changes or depends on batch handling in sc-decision-runner or decide (batch types, dispatch, scheduling, worker or thread counts, batch throughput)."
      },
      "instructions": "The state holds one planning item (`item.title`, `item.text`) and one anchor (`anchor.statement`, `anchor.scope`). The anchor governs how sc-decision-runner or decide handles a batch of decision requests: how a batch's requests are dispatched, scheduled or executed. Does the work in `item.text` plan, change, or depend on that batch handling? Judge subject overlap only; do not judge whether the item follows the anchor.",
      "type": "choice"
    }
  }
}
```
