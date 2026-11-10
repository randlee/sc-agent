# Requirement: Local runner timeouts

**Status:** Approved  
**Version:** 0.1.0  
**Created:** 2026-11-09  
**Last Updated:** 2026-11-09  
**Owner:** Rand Lee  

---

## REQ-SAG-0015: Local runner timeouts

**Status:** Approved  

### Requirement Statement

Definitions: The request timeout bounds one request to a runner. The load timeout bounds the time a local runner takes to load a model. The start budget is the 90-second limit on starting a local runner.

**MUST statements:**

- Each local runner MUST have its own request timeout, configurable per runner in the global config, with a default of 300 seconds; remote providers keep their 120-second default.
- Each local runner MUST have a load timeout, configurable per runner in the global config, with a default of 600 seconds.
- When discovery reports the model is not loaded, sc-agent MUST wait for the model to load before the first send, bounded by the load timeout and ending at once on cancel.
- For a daemon that loads its model only on its first request, the deadline of that first request MUST be the load timeout plus the request timeout.
- The request timeout MUST be within 10..=3600 seconds and the load timeout within 10..=7200 seconds, both checked at config load.

**MUST NOT statements:**

- Time a local runner spends loading a model MUST NOT count toward the request timeout or the 90-second start budget.
- Holdoff and the load wait MUST NOT use sleep().

### Rationale

Local models are slower than remote providers and loading a model can take minutes; counting load time against the request timeout fails requests that would succeed.

### Success Criteria

**Acceptance Criteria:**

- [ ] A local runner with no timeouts configured uses a 300-second request timeout and a 600-second load timeout.
- [ ] A `request_timeout_secs` of 900 configured for one runner is used for that runner only.
- [ ] A model reported not loaded is waited for, and the request's own timeout starts only after the model reports loaded.
- [ ] A load that exceeds the load timeout ends with a typed error naming the runner.
- [ ] Cancelling during the load wait ends it at once.
- [ ] A request or load timeout outside its bounds fails config load with an error naming the key path.

### Dependencies

**Related:**

- REQ-SAG-0801
- REQ-SAG-0013
- REQ-SAG-0601
- REQ-SAG-0010