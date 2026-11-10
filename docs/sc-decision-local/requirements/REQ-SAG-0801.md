# Requirement: Local decision runners come from configuration

**Status:** Draft  
**Version:** 0.1.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-09  
**Owner:** Rand Lee  

---

## REQ-SAG-0801: Local decision runners come from configuration

**Status:** Draft  

### Requirement Statement

Definitions: A local decision runner is a daemon on the user's machine or network that serves a decision model through the Jev API. Discovery is asking a runner whether it is up and which models it has loaded.

**MUST statements:**

- Each local decision runner MUST be defined in the global configuration by name, host, port and model.
- A local decision runner MUST be selectable by its configured name wherever a decision model is selected.
- `sc-decision-local` MUST send requests to a local decision runner in the Jev API format.
- `sc-decision-local` MUST report, through discovery, whether a configured runner is up and whether its configured model is loaded.

**MUST NOT statements:**

- `sc-decision-local` MUST NOT hard-code the host or port of any runner.

### Rationale

People are building local runners that duplicate the Jev API, and they differ only in where they run and how they report their loaded models.

### Success Criteria

**Acceptance Criteria:**

- [ ] A runner added to the global configuration is selected by its name with no rebuild.
- [ ] Discovery reports a runner that is not listening as down.
- [ ] Discovery reports a runner that is up without the configured model as up with the model not loaded.
- [ ] A workspace search finds no host or port literal for a local runner outside tests.

### Dependencies

**Related:**

- ADR-SAG-0001
- REQ-SAG-0201