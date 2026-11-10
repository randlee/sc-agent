# Requirement: Input size limits

**Status:** Approved  
**Version:** 0.1.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-09  
**Owner:** Rand Lee  

---

## REQ-SAG-0014: Input size limits

**Status:** Approved  

### Requirement Statement

**MUST statements:**

- One request MUST be at most 8 MiB.
- A batch MUST be at most 10,000 lines and at most 256 MiB.
- Each image MUST be at most 20 MiB.
- A request MUST carry at most 20 images.
- Input over a limit MUST be refused with a typed error that names the limit.

### Rationale

Bounded input keeps memory use predictable and turns an oversized input into a clear error before any send.

### Success Criteria

**Acceptance Criteria:**

- [ ] A request over 8 MiB is refused with a typed error naming the request size limit.
- [ ] A batch of 10,001 lines is refused with a typed error naming the line limit.
- [ ] A batch over 256 MiB is refused with a typed error naming the batch size limit.
- [ ] An image over 20 MiB is refused with a typed error naming the image size limit.
- [ ] A request with 21 images is refused with a typed error naming the image count limit.

### Dependencies

**Related:**

- REQ-SAG-0504
- REQ-SAG-0002