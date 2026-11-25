# Requirement: Question digest computed by sc-sha

**Status:** Approved  
**Version:** 0.2.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-24  
**Owner:** Rand Lee  

---

## REQ-SAG-0202: Question digest computed by sc-sha

**Status:** Approved  

### Requirement Statement

Definitions: The information is everything sent to the decision model with a question, other than the questions themselves. A single question is one question of the request. The question digest is the SHA-256 digest of the information plus one single question. sc-sha is the SHA-256 library sc-agent uses.

The result cache is specified by REQ-SAG-0703. Its key is the hash of the full request text and the requested model, not the question digest.

**MUST statements:**

- For every question sent to a decision model, sc-agent MUST compute a question digest over 100% of the information sent plus that single question.
- The question digest MUST be computed by sc-sha.
- The question digest MUST stay a hash of content only, with no model id added.
- The question digest MUST be recorded with that question's response, so the response can be retrieved by its digest.

**MUST NOT statements:**

- sc-agent MUST NOT compute the question digest with any SHA-256 implementation other than sc-sha.

### Rationale

A response must be retrievable from the exact information and question that produced it. Computing every digest with one library keeps digests from different crates and tools equal.

### Success Criteria

**Acceptance Criteria:**

- [ ] Two sends of the same information and the same single question record the same question digest.
- [ ] Changing any part of the information, or the question, changes the question digest.
- [ ] A request with three questions records three question digests, one per question.
- [ ] A workspace search finds SHA-256 computed only through sc-sha.

### Dependencies

**Related:**

- REQ-SAG-0004
- REQ-SAG-0703