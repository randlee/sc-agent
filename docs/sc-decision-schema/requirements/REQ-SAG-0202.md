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

Definitions: sc-agent is the product provided by this repository. A decision model is a model used for decision requests, such as Jev. The information is everything sent to the decision model with a question, other than the questions themselves. A single question is one question of the request. The question digest is the SHA-256 digest of the information plus one single question. To record a question digest with a response is to store the digest in the same entry as that response, so that reading the entry gives both the digest and the response.

The result cache is specified by REQ-SAG-0703. Its key is the hash of the full request text and the requested model, not the question digest.

**MUST statements:**

- When sc-agent sends a question to a decision model, sc-agent MUST compute the question digest of that question.
- sc-agent MUST record each question digest with the decision model's response to the question the digest was computed from.

### Rationale

Recording the digest with the response lets a response be retrieved from the exact information and question that produced it.

### Success Criteria

**Acceptance Criteria:**

- [ ] A unit test shows two sends of the same information and the same single question record the same question digest.
- [ ] A unit test shows that changing any part of the information, or the question, changes the question digest.
- [ ] A unit test shows a request with three questions records three question digests, one per question.

### Dependencies

**Related:**

- REQ-SAG-0004
