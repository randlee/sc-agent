# Requirement: Linked decision-tree walk records

**Status:** Draft  
**Version:** 0.1.0  
**Created:** 2026-11-24  
**Last Updated:** 2026-11-24  
**Owner:** Rand Lee  

---

## REQ-SAG-0024: Linked decision-tree walk records

**Status:** Draft  

### Requirement Statement

Definitions: an injected tree is caller-owned tree data identified by `tree_id` and `tree_sha256`; the caller's callback authors prompts and selects the next node. A walk is one traversal within a Run, with a Rust-minted ULID `walk_id`. A step records visited-node order, not a copy of the tree or of any session.

- The generated client MUST own one reusable traversal/logging path over an injected tree and callback; no tool-name or tree-name dispatch is permitted.
- It MUST write `walk.start` with the common header, `walk_id`, `tree_id` and `tree_sha256`.
- It MUST write one `walk.step` per visited node, with `walk_id`, zero-based `seq`, `prev_seq` (null initially), `node_id`, returned `session_id`s in statement order, `matched_rule`, `next_node` and a terminal `verdict` when present. Cached queries MUST reference the older session id actually returned.
- It MUST write `walk.end` with `walk_id`, the terminal verdict, rule rank and visited-node path. Failure/cancellation MUST end with its error code and the completed path; a failed callback MUST NOT fabricate a successful verdict.
- All three records MUST use the Run's common header and Logger. A tool-specific record MUST link the walk to the tool's judged item; the shared record MUST NOT contain a tool item id.
- Rust MUST mint walk ids; Python MUST NOT implement ULID generation. Fields passed across the wheel boundary MUST use named Rust types and typed pyclasses.
- Tool records MUST reference sessions; they MUST NOT duplicate requests, answers or attempts. Dry runs MUST write no walk event. Existing tree decision semantics MUST remain owned by the consumer callback.
- Invalid walk definitions or callback results MUST return `audit.invalid_walk`, category `validation`, naming the invalid node or field, with action to correct the injected tree/callback. Log failures MUST preserve the original log error.

### Rationale

Linked, generic events let both current tools and future tools use arbitrary trees without copying session data or traversal logging.

### Success Criteria

**Acceptance Criteria:**

- [ ] A two-node injected tree with two queries at the first node yields contiguous step sequence, the correct prev links, returned session ids and a single terminal path.
- [ ] A different tree and callback work without changing a tool-name table or the shared algorithm.
- [ ] Cached sessions retain their original ids; callbacks with no queries write an empty session-id list.
- [ ] Failure and cancellation end the walk once, preserve completed steps and error code, and do not write a successful terminal verdict.
- [ ] No session data or tree definition appears in walk events, and a dry run writes nothing.

### Dependencies

**Related:**

- REQ-SAG-0018
- REQ-SAG-0005
- NFR-SAG-0004
- NFR-SAG-0005
- ADR-SAG-0019
- ADR-SAG-0021
