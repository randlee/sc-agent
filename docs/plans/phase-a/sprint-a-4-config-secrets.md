<!-- Generated from bead agt-4p2.4 for file-based reviewers. The bead is the source of truth: edit the bead, then re-export. -->

# Sprint A-4: sc-agent-config and sc-agent-secrets
- id: `a-4` (bead `agt-4p2.4`)
- branch: `sprint/a-4-config-secrets`
- depends on: agt-4p2.1
- copy script: `docs/plans/phase-a/copy/a-4.sh`

## Description

Global config loaded with sc-config (JSON) from ~/.sc-agent/: extra models, optional log_root (default ~/.sc-agent), workers (default 8), max_sends (default 5), local runners (kind, host, port, model, optional key_env). Secrets: credential env-var names (incl. LAYA_API_KEY) and key resolution per provider or configured variable: process env, then repo-root .env, then keyring; never log a key. Records: REQ-SAG-0003, REQ-SAG-0102, REQ-SAG-0401, REQ-SAG-0601, REQ-SAG-0701, REQ-SAG-0801.

## Design

# Sprint A-4: sc-agent-config and sc-agent-secrets (bead agt-4p2.4)

Copy script: `docs/plans/phase-a/copy/a-4.sh` (sources `lib.sh`; source sc-agent-ref@dce239a). Dry run 2026-11-09 (re-checked): 13 copy steps, 8 files.
Targets: `crates/sc-agent-config` (new code) and `crates/sc-agent-secrets`. Layer: directly above sc-agent-types (ADR-SAG-0004); both depend only on A-1.
Governing: REQ-SAG-0102, REQ-SAG-0401, REQ-SAG-0601 (max sends configurable, default 5), REQ-SAG-0701 (workers, default 8), REQ-SAG-0801 (each local decision runner in the global config by name, host, port and model), REQ-SAG-0002, REQ-SAG-0003 and REQ-SAG-0005 (`<log_root>`, default `~/.sc-agent`), REQ-SAG-0009, REQ-SAG-0010, ADR-SAG-0004.
Owner decisions: owner-architecture (sc-config, JSON), owner-model-catalog, owner-busy-resend, owner-local-discovery (ports in the config file), owner-local-decision-crate, owner-secrets-env-vars, owner-provider-and-model, owner-error-contract, owner-lowlevel-no-tokio.

## sc-config 0.2.0 API (checked in ~/.cargo/registry, repo randlee/sc-runtime)

- `AppName::new(&str) -> Result<AppName, ConfigError>`; `sc-agent` gives env prefix `SC_AGENT__`.
- `Loader::new(app, config_dir: PathBuf, environment: Vec<(OsString, OsString)>).load::<T: DeserializeOwned>()`: reads `<dir>/default.json` (required), merges optional `<dir>/local.json`, applies `SC_AGENT__A__B=value` overrides (segments lowercased; value parsed as JSON, else a string).
- `ConfigError` codes: CFG.MISSING_DEFAULT, CFG.READ, CFG.PARSE, CFG.OVERRIDE, CFG.DESERIALIZE, APP.INVALID_NAME, CFG.NON_UNICODE; has `code()`, Display, `source()`; not Clone.
- Overrides walk objects only, so `models` and `local_runners` are objects keyed by id or name (`SC_AGENT__LOCAL_RUNNERS__LAYA__PORT=9000`).

## 1. Copy list (a-4.sh)

| Source (sc-agent-ref) | Lines | Target | What |
|---|---|---|---|
| crates/sc-agent-secrets/LICENSE | all | sc-agent-secrets/LICENSE, sc-agent-config/LICENSE | MIT |
| sc-agent-secrets/src/resolve.rs | 17-24 | sc-agent-secrets/src/vars.rs | TYPESAFE/ANTHROPIC/OPENAI_API_KEY consts |
| sc-agent-secrets/src/resolve.rs | 34-42 | sc-agent-secrets/src/env.rs | EnvSource trait |
| sc-agent-secrets/src/platform.rs | 15-31 | sc-agent-secrets/src/env.rs | ProcessEnv |
| sc-agent-types/src/secret.rs | 9-134 | sc-agent-secrets/src/api_key.rs | ResolvedApiKey (becomes ApiKey) |
| sc-agent-types/src/secret.rs | 136-194 | sc-agent-secrets/src/api_key.rs | unit tests |
| sc-agent-secrets/src/resolve.rs | 305-348 | sc-agent-secrets/tests/resolve.rs | MapEnv test double |
| sc-agent-secrets/src/dotenv.rs | 1-241 | sc-agent-secrets/src/dotenv.rs | repo-root .env parser and its tests |
| sc-agent-secrets/src/resolve.rs | 44-59 | sc-agent-secrets/src/keyring.rs | KeyringSource trait |
| sc-agent-secrets/src/platform.rs | 33-264 | sc-agent-secrets/src/keyring.rs | PlatformKeyring, ReadGate (15 s deadline by recv_timeout, no sleep), read_entry, classify_keyring, sanitized causes, current_account |
| sc-agent-secrets/src/platform.rs | 265-413 | sc-agent-secrets/src/keyring.rs | keyring unit tests |
| sc-agent-secrets/src/resolve.rs | 350-400 | sc-agent-secrets/tests/resolve.rs | MapKeyring test double |

`a-4.sh` does not yet have the last five rows; add them before the sprint starts.

Not copied: `ANTHROPIC_KEYCHAIN_SERVICE` (the keyring service is the variable name), the `*_logged` functions, test_support.rs; `redact_error`, `JevError::map_detail`; the old TOML `models.toml` reader (sc-jev-host/src/paths.rs:266-282).

## 2. Required edits after copying

- vars.rs: keep the three consts; add `OPENROUTER_API_KEY`, `TOGETHER_API_KEY`, `NOUS_API_KEY` (the providers' usual names; assistant decision) and `LAYA_API_KEY` (Laya's own variable, laya/serve.py). Add `pub const fn env_var(provider: Provider) -> &'static str` (exhaustive match over the six) and `pub const ALL_VARS: &[&str]` (all seven). The only credential-name literals in the workspace (REQ-SAG-0401).
- env.rs: drop ADR-SAG-0020; a present empty value is `Some("")` (it skips `.env` and goes to the keyring, as before); a non-Unicode value is `Some("")`.
- dotenv.rs: `JevError::io_failed_with` -> `SecretsError::DotenvRead { kind, cause }` (cause = the sanitized label); `load_dotenv(repo_root)` unchanged otherwise.
- keyring.rs: `JevError` -> `SecretsError::KeyringRead { kind, cause }` (sanitized cause: keyring errors can carry secret bytes) and `SecretsError::KeyringTimeout`; `KeyringSource::password` returns `Result<Option<String>, SecretsError>`; drop the `# Panics` text and ADR-SAG-0020; the reader thread uses `thread::Builder` (no `thread::spawn`).
- api_key.rs: `ResolvedApiKey` -> `ApiKey`; `pub(crate) fn try_new(var: &str, raw: String) -> Result<Self, SecretsError>`; empty -> `KeyMissing { var }`; whitespace/control/non-visible-ASCII -> `KeyMalformed { var, reason }`; recovery text names the process environment, the repo-root `.env` file and the keyring. Delete `redact_error`. Keep `as_str` (only sc-agent-http's transport reads it) and `redact_text`. Port the unit tests to `Provider::ALL`/`env_var` and the new ids.
- tests/resolve.rs: drop `#[cfg(test)]` on MapEnv and MapKeyring; `use sc_agent_secrets::{EnvSource, KeyringSource}`.

## 3. Target public API (exact; bodies elided)

```rust
// ===== sc-agent-secrets =====
mod api_key; mod dotenv; mod env; mod error; mod keyring; mod resolve; mod vars;
pub use api_key::ApiKey;
pub use env::{EnvSource, ProcessEnv};
pub use error::SecretsError;
pub use keyring::{KeyringSource, PlatformKeyring};
pub use resolve::{resolve, resolve_var, KeySource, KeySources, ResolvedKey};
pub use vars::{env_var, ALL_VARS, ANTHROPIC_API_KEY, LAYA_API_KEY, NOUS_API_KEY, OPENAI_API_KEY, OPENROUTER_API_KEY, TOGETHER_API_KEY, TYPESAFE_API_KEY};

pub trait EnvSource: Send + Sync { fn var(&self, name: &str) -> Option<String>; }
pub trait KeyringSource: Send + Sync { fn password(&self, service: &str, account: &str) -> Result<Option<String>, SecretsError>; }
#[derive(Debug, Default, Clone, Copy)]
pub struct PlatformKeyring;                                     // service = variable name, account = OS user
impl KeyringSource for PlatformKeyring {}
#[derive(Debug, Default, Clone, Copy)]
pub struct ProcessEnv;
impl EnvSource for ProcessEnv {}

#[must_use = "read the secret with as_str only at the transport that sends it"]
#[derive(Clone, PartialEq, Eq)]
pub struct ApiKey(Arc<str>);
impl ApiKey {
    #[must_use] pub fn as_str(&self) -> &str;
    #[must_use] pub fn redact_text(&self, text: &str) -> String;   // replaces the key with "[redacted]"
}
impl fmt::Debug for ApiKey {}    // "ApiKey([redacted])"
impl fmt::Display for ApiKey {}  // "[redacted]"

/// Where keys are looked up, in this order (REQ-SAG-0401): process environment, `<repo_root>/.env`, keyring.
pub struct KeySources<'a> { pub env: &'a dyn EnvSource, pub repo_root: Option<&'a Path>, pub keyring: &'a dyn KeyringSource }
impl<'a> KeySources<'a> {
    /// ProcessEnv + PlatformKeyring; `repo_root` is the git work-tree top level (sc_agent_log::Repo::work_tree_root), None outside git (no .env read).
    pub fn system(repo_root: Option<&'a Path>) -> Self;
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeySource { Env, DotEnv, Keyring }
impl KeySource { pub const fn as_str(self) -> &'static str; }   // "env" | ".env" | "keyring"
#[derive(Debug, Clone)]
pub struct ResolvedKey { pub key: ApiKey, pub source: KeySource }   // source: decide doctor lists it (REQ-SAG-0503)
/// The key for `provider`, from its variable: env, then .env (a variable already set in env is not overridden), then keyring.
pub fn resolve(provider: Provider, sources: &KeySources<'_>) -> Result<ResolvedKey, SecretsError>;
/// The same order for a variable named by configuration (a local runner's `key_env`, e.g. LAYA_API_KEY).
pub fn resolve_var(var: &str, sources: &KeySources<'_>) -> Result<ResolvedKey, SecretsError>;

// ===== sc-agent-config =====
mod config; mod error;
pub use config::{default_dir, load, AgentConfig, ExtraModel, LocalRunner, APP_NAME, DEFAULT_MAX_SENDS, DEFAULT_WORKERS};
pub use error::AgentConfigError;

pub const APP_NAME: &str = "sc-agent";                      // env prefix SC_AGENT__
pub const DEFAULT_WORKERS: NonZeroUsize;                    // 8: NonZeroUsize::MIN.saturating_add(7)
pub const DEFAULT_MAX_SENDS: NonZeroU32;                    // 5: NonZeroU32::MIN.saturating_add(4)

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]   // Default: no models, no log_root, workers 8, max_sends 5, no runners
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct AgentConfig {
    #[serde(default)] pub models: BTreeMap<ModelId, ExtraModel>,          // extra catalog rows (REQ-SAG-0102)
    #[serde(default)] pub log_root: Option<PathBuf>,                      // REQ-SAG-0003/0005; absent -> default_dir()
    #[serde(default = "default_workers")] pub workers: NonZeroUsize,      // REQ-SAG-0701, default 8
    #[serde(default = "default_max_sends")] pub max_sends: NonZeroU32,    // REQ-SAG-0601, default 5, one value for all providers
    #[serde(default)] pub local_runners: BTreeMap<String, LocalRunner>,   // keyed by runner name (REQ-SAG-0801)
}
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct ExtraModel { pub line: ShortName, pub released: ReleaseDate }
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct LocalRunner {
    /// Daemon kind, parsed by sc-decision-local's DaemonKind ("strands" | "laya" | "mapika" | "generic").
    pub kind: String,
    pub host: String,
    pub port: u16,                                                        // owner-local-discovery
    pub model: ModelId,
    #[serde(default)] pub key_env: Option<String>,                        // resolved with sc_agent_secrets::resolve_var
}
impl Default for AgentConfig {}

/// `~/.sc-agent`: the config directory and sc-observability's standard default log root for app sc-agent
/// (`~/.<app>`); std::env::home_dir(); none -> HomeUnknown.
pub fn default_dir() -> Result<PathBuf, AgentConfigError>;

/// Loads `<config_dir>/default.json` (+ optional local.json) and SC_AGENT__* overrides from `environment`
/// (decide passes default_dir()). No default.json -> AgentConfig::default(), unless an SC_AGENT__ variable is set
/// (then config.load with cause CFG.MISSING_DEFAULT, so an override is never ignored). A `local_runners` name equal
/// to a short name is RunnerName.
pub fn load(config_dir: PathBuf, environment: Vec<(OsString, OsString)>) -> Result<AgentConfig, AgentConfigError>;
impl AgentConfig {
    /// `log_root`, else default_dir() (REQ-SAG-0003).
    pub fn log_root(&self) -> Result<PathBuf, AgentConfigError>;
    /// Catalog::builtin() plus every `models` entry, through Catalog::with_config_model.
    pub fn catalog(&self) -> Result<Catalog, AgentConfigError>;
}
```

Example `default.json`:
```json
{ "workers": 8, "max_sends": 5,
  "models": { "claude-haiku-5-6": { "line": "haiku", "released": "2026-12-01" } },
  "local_runners": { "laya": { "kind": "laya", "host": "127.0.0.1", "port": 8000, "model": "convaiinnovations/laya", "key_env": "LAYA_API_KEY" } } }
```
Invalid values (empty model id, `workers: 0`, unknown short name, bad date) fail inside sc-config as CFG.DESERIALIZE; the message carries the A-1 TypesError text with its id. `log_root` is optional (default `~/.sc-agent`). An unknown `kind` is caught by sc-decision-local when the runner is built (`local.unknown_daemon`).

## 4. Full error enums (new; ErrorId from A-1, Display = detail(), no thiserror, never key bytes)

```rust
// sc-agent-secrets/src/error.rs
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum SecretsError {
    /// The variable is unset, empty or blank; the message names the variable and how to set it.
    KeyMissing { var: String },                         // "secrets.key_missing"
    /// The key has whitespace, a control character or a non-visible-ASCII character.
    KeyMalformed { var: String, reason: &'static str }, // "secrets.key_malformed"
    /// The repo-root .env exists and cannot be read; cause = sanitized io label.
    DotenvRead { kind: String, cause: &'static str },   // "secrets.dotenv_read"
    /// The platform keyring could not be read; cause = sanitized label (never the keyring's own text).
    KeyringRead { kind: String, cause: &'static str },  // "secrets.keyring_read"
    /// The keyring did not answer within 15 s (or an earlier read is still stuck).
    KeyringTimeout,                                     // "secrets.keyring_timeout"
}
impl ErrorId for SecretsError {}   // cause() = None

// sc-agent-config/src/error.rs
#[derive(Debug)]
#[non_exhaustive]
pub enum AgentConfigError {
    /// sc-config failed; cause = ErrorDetail::foreign(inner.code(), &inner), e.g. id "CFG.PARSE".
    Load(sc_config::ConfigError),                       // "config.load"
    /// A `models` entry conflicts with the catalog; cause = the TypesError detail (types.model_on_two_lines).
    Catalog(sc_agent_types::TypesError),                // "config.catalog"
    /// No home directory for default_dir().
    HomeUnknown,                                        // "config.home_unknown"
    /// A local runner is named like a built-in short name (assistant decision).
    RunnerName { name: String },                        // "config.runner_name"
}
impl ErrorId for AgentConfigError {}
impl From<sc_config::ConfigError> for AgentConfigError {}
impl From<sc_agent_types::TypesError> for AgentConfigError {}
// source() = Some(inner)
```

## 5. New code

- Both crates: `Cargo.toml` (`[lints] workspace = true`, no features), `README.md`, `src/lib.rs`, `src/error.rs`.
- secrets `src/resolve.rs` (~50 lines): `resolve` = `resolve_var(env_var(provider), sources)`; `resolve_var` follows the old `resolve_in_order`: env value present and non-blank -> Env; env unset -> `.env` under `repo_root` (if any) -> DotEnv; then the keyring (service = variable name, account = OS user) -> Keyring; none -> KeyMissing; each value through `ApiKey::try_new`. No logging: config and secrets return errors only and callers log them (assistant decision).
- config `src/config.rs` (~80 lines): structs, default fns, `default_dir`, `log_root`, `load` (`AppName::new(APP_NAME)?`, missing default.json rule, `Loader::new(..).load::<AgentConfig>()?`, runner-name check), `catalog` (fold over `models`).
- Tests, secrets (`tests/resolve.rs`, MapEnv, MapKeyring, tempdir .env): env wins over .env and keyring; .env wins over keyring; keyring last; a blank env value skips .env; `repo_root: None` reads no .env; each result names its KeySource; an unreadable .env -> secrets.dotenv_read; a failing keyring -> secrets.keyring_read with no secret bytes; each Provider resolves from its own variable; unset/empty/blank -> secrets.key_missing naming the variable; malformed -> secrets.key_malformed; no error, Debug or Display contains the key; `resolve_var(LAYA_API_KEY, ..)` works; `ALL_VARS` contains every `env_var(p)` and LAYA_API_KEY; `ApiKey`, `MapEnv`, `ProcessEnv` are Send + Sync.
- Tests, config (`tests/load.rs`, tempdir + explicit environment Vec, never the process env): defaults (workers 8, max_sends 5, no models, no runners, log_root None -> default_dir()); no default.json -> defaults, but with SC_AGENT__WORKERS=2 set -> config.load (CFG.MISSING_DEFAULT); a runner named `jev` -> config.runner_name; `SC_AGENT__MAX_SENDS=2` gives 2; local.json overrides default.json; later `haiku` model makes `catalog()?.resolve(Latest(Haiku))` return it, earlier does not; `claude-sonnet-5-5` under `haiku` -> config.catalog with cause types.model_on_two_lines; missing default.json -> config.load with cause CFG.MISSING_DEFAULT; bad JSON -> cause CFG.PARSE with the path; `workers: 0` and an unknown key -> CFG.DESERIALIZE; a `local_runners` entry loads kind, host, port, model and key_env; `SC_AGENT__LOCAL_RUNNERS__LAYA__PORT=9000` overrides the port.
- Workspace gate (REQ-SAG-0401): a test greps for every name in ALL_VARS and fails if one appears as a literal outside `crates/sc-agent-secrets`.

## 6. Dependencies

| Crate | Version | Used by |
|---|---|---|
| sc-agent-types | path, A-1 | both |
| sc-config | "0.2.0" (crates.io) | config |
| serde | "1", features ["derive"] | config |
| keyring | "3.6", no default features; per-target features as in sc-agent-ref (apple-native, windows-native, sync-secret-service + crypto-rust + vendored) | secrets |
| whoami | "1.6" | secrets (keyring account) |
| dev: tempfile | "3" | config and secrets tests |
| dev: serde_json | "1" | config tests |

No tokio (keyring's sync Secret Service backend) and no sc-observability.

## Acceptance criteria

- [ ] Copy first: the sprint's first commit is the unchanged output of `docs/plans/phase-a/copy/a-4.sh` (sc-agent-ref@dce239a), committed before any edit; every change needed to make that code build and pass comes in later commits (owner-copy-mechanically).
- [ ] REQ-SAG-0102 AC1/AC2: a default.json models entry for a haiku model dated later than every built-in haiku makes AgentConfig::catalog()?.resolve(Latest(Haiku)) return it with no rebuild; an earlier date does not (tempdir tests).
- [ ] REQ-SAG-0102: a user adds a model by editing JSON only; keys models, log_root (optional), workers, max_sends, local_runners load through sc-config 0.2.0 Loader with SC_AGENT__ env overrides (tests: SC_AGENT__MAX_SENDS=2, SC_AGENT__LOCAL_RUNNERS__LAYA__PORT=9000).
- [ ] REQ-SAG-0601: this sprint delivers only the max_sends setting (default 5; 2 via file or env gives 2; 0 rejected); resending on 429/502/529 is built in A-5 and A-9.
- [ ] REQ-SAG-0701: this sprint delivers only the workers setting (default 8; 0 rejected); running a batch across that many workers is built in A-9.
- [ ] REQ-SAG-0801 (config part): each local_runners entry carries name (map key), kind, host, port, model and optional key_env from the config file; a runner added to default.json is returned by load() with no rebuild; no runner host or port literal outside tests (grep).
- [ ] REQ-SAG-0401: TYPESAFE_API_KEY, ANTHROPIC_API_KEY, OPENAI_API_KEY, OPENROUTER_API_KEY, TOGETHER_API_KEY, NOUS_API_KEY and LAYA_API_KEY appear as string literals only in crates/sc-agent-secrets (automated check).
- [ ] REQ-SAG-0401: resolve(p, sources) and resolve_var(var, sources) look in the process environment, then the repo-root .env, then the platform keyring, and report which source supplied the key; each of the six Providers uses env_var(p); none -> secrets.key_missing naming the variable; malformed -> secrets.key_malformed (MapEnv/MapKeyring/tempdir tests; process env never mutated).
- [ ] REQ-SAG-0003: AgentConfig::log_root() returns the configured log_root, else default_dir() = ~/.sc-agent (sc-observability's default for app sc-agent); the config directory decide passes is default_dir().
- [ ] A missing default.json gives AgentConfig::default() unless an SC_AGENT__ override is set (then config.load); a local runner named like a short name is config.runner_name (tests).
- [ ] Never log or print a key: ApiKey Debug/Display print [redacted]; no SecretsError Display/Debug contains key bytes, including keyring failures (test).
- [ ] REQ-SAG-0002: SecretsError and AgentConfigError implement ErrorId with ids secrets.* and config.*; bad JSON gives config.load whose cause has id CFG.PARSE and the sc-config message with the path; a model on two lines gives config.catalog whose cause is types.model_on_two_lines (tests); clippy with the workspace denies passes.
- [ ] REQ-SAG-0009/0010: cargo tree for both crates lists no tokio crate; no sleep call; dependencies limited to sc-agent-types, sc-config, serde, keyring, whoami (dev: tempfile, serde_json).

## Notes

Closed 2026-11-09 by owner answers (search-for-truth step 7); decisions are in the design:
- Q1 config location: ~/.sc-agent/ (default_dir); decide passes it.
- Q2 .env and keyring: restored; order process env -> repo-root .env -> keyring (REQ-SAG-0401).
- Q3 variable names: OPENROUTER_API_KEY, TOGETHER_API_KEY, NOUS_API_KEY (assistant).
- Q4 log_root: optional, default ~/.sc-agent (REQ-SAG-0003).
- Q5 a local runner may not be named like a short name (config.runner_name, assistant).
- Q6 config and secrets log nothing; callers log their errors (assistant).
Follow-up: docs/plans/phase-a/copy/a-4.sh needs the five added copy rows (.env, keyring, MapKeyring).
No open questions.
