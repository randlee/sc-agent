# Decision: sc-publish kit owns release surface

**Status:** Approved  
**Version:** 0.1.0  
**Created:** 2026-11-11  
**Last Updated:** 2026-11-11  
**Owner:** Rand Lee  
**Decision Date:** 2026-11-11  

---

## ADR-SAG-0018: sc-publish kit owns release surface

**Status:** Approved  

### Related Documents

**Requirements:**

- REQ-SAG-0130
- REQ-SAG-0131
- REQ-SAG-0132

### Context

Release workflows, channel contracts, and `publish-artifacts.toml` are rendered from consumer input by [sc-publish](https://github.com/randlee/sc-publish).

### Decision

Edit `release/sc-publish-consumer-input.json` and re-run `install.py` from the pinned kit; do not hand-edit rendered files under `release/` or kit-owned `.github/workflows` publish YAML except through the installer.

**Key Principles:**

- Pin a single qualified 40-character kit SHA in `release/sc-publish-pin.toml`.
- Consumer input is the sole source of crate order, binaries, channels, and optional **docli** preflight checks.

### Consequences

**Positive:**

- Kit upgrades flow through pin bump + dry-run.

**Negative:**

- Drift between JSON and rendered TOML is a hard fail until re-install.
