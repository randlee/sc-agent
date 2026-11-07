# sc-agent-http

Shared blocking HTTP layer for the sc-agent backends (`sc-jev`,
`sc-jev-backends`): request and response types, the `Transport` trait, the
ureq-based `UreqTransport`, busy retry with cancellation, the 8 MiB response
cap, and redaction of the API key from every error.

It carries bytes only. Each backend crate owns its wire format.

Part of the [sc-agent](https://github.com/randlee/sc-agent) workspace. See the
repository `docs/` for architecture and requirements. Licensed under MIT.
