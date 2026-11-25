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

The shared client MUST record arbitrary injected decision-tree walks as linked start, step and end records within a Run, referencing sessions without copying their query or answer data.

### Rationale

Linked records support different tools and trees without duplicating session data (D3, D8, D12).

### Dependencies

**Related:**

- REQ-SAG-0018
- ADR-SAG-0019
- ADR-SAG-0021
