```json
{
  "questions": {
    "addressed": {
      "criteria": {
        "no": "The item says nothing about limits on a crate's dependencies.",
        "unclear": "The item mentions limits in words too vague to tell whether they are the anchor's restriction.",
        "yes": "The item says what a crate is limited to depending on, or names a crate it does not depend on, consistent with the layer restriction."
      },
      "instructions": "`anchor.statement` is a restriction on Cargo.toml dependencies. Does `item.text` itself carry that restriction as a constraint, in its own words, by saying what a crate is limited to depending on or what it does not depend on? Silence about limits is no, even when the dependency the item adds is allowed. Judge presence of the restriction only.",
      "type": "choice"
    },
    "contradicted": {
      "criteria": {
        "no": "Every dependency the item plans is on the same or a lower layer, or the item plans no dependency.",
        "unclear": "The item plans a dependency but the crates or their layers cannot be determined from the text.",
        "yes": "The item makes some sc-agent crate depend on an sc-agent crate in a higher layer."
      },
      "instructions": "Using the layer order in `anchor.statement` (bottom to top), does `item.text` plan for an sc-agent crate to depend, in any Cargo.toml dependency section, on an sc-agent crate in a higher layer than its own? Dependencies on the same or a lower layer are not conflicts. Calling a dependency temporary or an exception does not remove a conflict. Silence about dependencies is not a conflict. Judge conflict only.",
      "type": "choice"
    },
    "relevant": {
      "criteria": {
        "no": "The item adds, removes, changes and lists no dependency between sc-agent crates, for example implementation, test or documentation work, including documentation or diagrams about the crate layering.",
        "unclear": "The item is too vague to tell whether it adds or changes a dependency between sc-agent crates.",
        "yes": "The item adds, removes, changes or explicitly lists a dependency between sc-agent crates."
      },
      "instructions": "The state holds one planning item (`item.title`, `item.text`) and one anchor (`anchor.statement`, with the activity it governs in `anchor.scope`). Does the work in `item.text` add, remove or change a dependency of one sc-agent crate on another sc-agent crate in a Cargo.toml, or explicitly state which sc-agent crates a crate depends on? Judge subject overlap only, not whether the item obeys the anchor.",
      "type": "choice"
    }
  }
}
```
