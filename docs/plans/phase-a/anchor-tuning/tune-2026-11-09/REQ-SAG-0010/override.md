```json
{
  "questions": {
    "addressed": {
      "criteria": {
        "no": "The item writes down no such commitment, whatever else it says about waiting, retries or timeouts.",
        "unclear": "The item contains wording that may or may not be a no-sleep commitment.",
        "yes": "The item writes down the no-sleep commitment or a named replacement for sleeping."
      },
      "instructions": "Does `item.text` contain an explicit commitment matching `anchor.statement`, namely that the sc-agent code will not sleep? Look for wording such as 'no sleep calls', a rule or lint forbidding sleep, or a named replacement for sleeping such as CancelToken::wait_until. Mentioning delays, retries, holdoffs or timeouts without that commitment does not count, and an item that sleeps does not count. Judge only whether the commitment is written in the item.",
      "type": "choice"
    },
    "contradicted": {
      "criteria": {
        "no": "The item states no sleep call and no permission for one in sc-agent Rust source.",
        "unclear": "The item states a call in sc-agent Rust source that might be a sleep function, but the text does not show which.",
        "yes": "The item states a sleep call, or permission for one, in sc-agent Rust source."
      },
      "instructions": "Judge only what `item.text` states. Does it state that sc-agent Rust code (tests included) will call std::thread::sleep, std::thread::sleep_until, tokio::time::sleep or another sleep function, or that such a call is allowed, which `anchor.statement` rules out? Do not infer a sleep from an unstated waiting mechanism: 'waits out the holdoff' or 'retries after a delay' without a named sleep is not a conflict. tokio::time::timeout and cancellation-token waits are not sleep functions. Sleep calls outside sc-agent Rust source are not a conflict. Silence is not a conflict.",
      "type": "choice"
    },
    "relevant": {
      "criteria": {
        "no": "The described work writes or edits no Rust code inside an sc-agent crate (for example docs only, CI files, or a script in another language).",
        "unclear": "The item text is too vague to tell whether Rust code in an sc-agent crate is written or edited.",
        "yes": "The described work writes or edits Rust code inside an sc-agent crate."
      },
      "instructions": "`anchor.scope` names the work the anchor governs: adding or changing Rust source in an sc-agent crate. Does `item.text` describe work within that scope? Count any Rust code in an sc-agent crate (library code, runners, clients, types or tests), whether or not it involves waiting or timing. Work that touches only documents, CI files, or non-Rust or non-sc-agent code is outside the scope. Judge whether the item is within scope only.",
      "type": "choice"
    }
  }
}
```
