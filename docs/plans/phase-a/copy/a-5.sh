#!/usr/bin/env bash
# Sprint A-5 (bead agt-4p2.5): copy the sc-agent-http starting code from sc-agent-ref@dce239a.
# Run: bash docs/plans/phase-a/copy/a-5.sh   (TARGET=<dir> to dry-run elsewhere)
#
# This script only copies text. It does NOT do any of the following; the developer does them
# (see the bead design for the exact target code):
#   - write Cargo.toml, README.md, src/lib.rs, src/error.rs (HttpError) or src/resend.rs (ResendPolicy,
#     Attempts, Next, send_once): the old retry loop is NOT copied, it sleeps (old retry.rs:247-268);
#   - replace every JevError / ProviderFailure / BusyCause / NetworkErrorKind with HttpError,
#     ProviderReply, BusyReason and NetworkKind (copied below under their old names);
#   - drop HeaderName (header names become String, validated by the ureq request builder, no assert!);
#   - Endpoint: drop the `https` flag, loopback_http() and its assert!s; accept http:// only for
#     loopback hosts (open question Q1); rewrite log_form without slice indexing;
#   - SecretHeader / ApiKey: ResolvedApiKey -> sc_agent_secrets::ApiKey; drop the test-util matches();
#   - Timeouts: drop the clamp (MIN/MAX_*) and its unit test; keep the 10 s / 120 s defaults;
#   - ureq_transport: drop loopback_for_tests, is_valid_url, the https_only refusal branch and every
#     cfg(feature = "test-util"); map errors to HttpError; port its unit tests;
#   - retry_after: drop report_seconds/REPORT_MAX_SECONDS (they round); add seconds() -> f64 unrounded;
#   - failure.rs: keep only the quota reader (Body, QUOTA_MARKERS, is_quota_reply) and read_failure;
#     drop api_failure, ReplyShape, says_input_too_long and their tests;
#   - retry.rs: keep only the constants, jitter, is_busy, backoff and the pure unit tests; Retry-After
#     holdoffs get no jitter (REQ-SAG-0601); drop MAX_SENDS (no hard cap) and WAIT_SLICE;
#   - test support: drop FakeClock/RecordingSleeper/with_send_duration; tests pass `now: Instant`;
#   - write tests/resend.rs (every REQ-SAG-0601/0602 acceptance case) from scratch.
. "$(dirname "$0")/lib.sh"

H=crates/sc-agent-http
Y=crates/sc-agent-types
T=crates/sc-agent-http

copy_file  $H/LICENSE                                   $T/LICENSE
# Method; SecretHeader, Endpoint, HttpRequest, HttpResponse (HeaderName 39-117 and tests not copied)
copy_lines $H/src/request.rs           1   37           $T/src/request.rs
copy_lines $H/src/request.rs         119  484           $T/src/request.rs
# Transport trait + Timeouts (clamp and its test are removed by hand)
copy_lines $H/src/transport.rs         1  128           $T/src/transport.rs
# Production ureq transport, LookupResolver, error classification, unit tests
copy_file  $H/src/ureq_transport.rs                     $T/src/ureq_transport.rs
# The one Retry-After parser
copy_file  $H/src/retry_after.rs                        $T/src/retry_after.rs
# Redaction of an error by the request's key
copy_lines $H/src/redact.rs            1   46           $T/src/redact.rs
# Quota-exhaustion reader (REQ-SAG-0602) and its documented-shape test
copy_lines $H/src/failure.rs           1   53           $T/src/quota.rs
copy_lines $H/src/failure.rs          61  177           $T/src/quota.rs
copy_lines $H/src/failure.rs         390  422           $T/src/quota.rs
# What a failing reply said (busy_reply/read_failure) and the key-scrub test
copy_lines $H/src/failure.rs         304  352           $T/src/reply.rs
copy_lines $H/src/failure.rs         627  652           $T/src/reply.rs
# ProviderFailure + bound_provider_text -> ProviderReply (src/reply.rs)
copy_lines $Y/src/error.rs            67   73           $T/src/reply.rs
copy_lines $Y/src/error.rs           108  247           $T/src/reply.rs
# NetworkErrorKind -> NetworkKind, BusyCause -> BusyReason (src/error.rs)
copy_lines $Y/src/code.rs            268  325           $T/src/error.rs
copy_lines $Y/src/code.rs            344  364           $T/src/error.rs
# Resend numbers, jitter, is_busy, backoff and pure unit tests (the loop 247-626 is not copied)
copy_lines $H/src/retry.rs            54   74           $T/src/resend.rs
copy_lines $H/src/retry.rs           231  245           $T/src/resend.rs
copy_lines $H/src/retry.rs           628  653           $T/src/resend.rs
copy_lines $H/src/retry.rs           663  713           $T/src/resend.rs
copy_lines $H/src/retry.rs           726  742           $T/src/resend.rs
# Scripted transport test double (FakeClock / sleeper not copied)
copy_lines $H/src/test_util.rs        18   30           $T/tests/support/mod.rs
copy_lines $H/src/test_util.rs       139  223           $T/tests/support/mod.rs
# Loopback transport tests: request helpers, assert_network, send_once, serve_once, real refused connection
copy_lines $H/tests/ureq.rs           22   54           $T/tests/ureq.rs
copy_lines $H/tests/ureq.rs          240  284           $T/tests/ureq.rs
# Resend test helpers (request/busy/ok builders)
copy_lines $H/tests/retry.rs          22   45           $T/tests/resend.rs
