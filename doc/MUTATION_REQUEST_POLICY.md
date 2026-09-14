# Mutation request policy: owner review handoff

This branch prepares the SDK boundary required by ig-engine's trading journal.
It is an unpublished change based on `d497ff5755696533e356fff8bf415951e750cd7e`.
The package version is unchanged. Owner review and a published release are still
required before ig-engine may consume it; ig-engine keeps its
`disabled|simulation` execution backends meanwhile.

## Public contract

`model::retry::RequestPolicy` is also exported by `prelude`:

- `Standard` preserves eligible finite status retries, key rotation, and one
  authentication replay for ordinary requests.
- `SingleAttempt` permits at most one business-request send per invocation. It
  disables the SDK retry loop, post-send key rotation, authentication replay,
  HTTP redirects, and reqwest protocol retries.

`HttpClient::request_with_policy(method, path, body, version, policy)` adds an
explicit policy without changing the seven `OrderService` mutation signatures or
their IG request/response DTOs. The generic `request`, `post`, `put`, `delete`, and
POST-with-DELETE-override helpers cannot weaken known trading mutations by
selecting `Standard`. Selection and dispatch use the same normalized URL.
Percent-escaped mutation paths are also protected conservatively, without
rewriting a caller's identifier.

| Operation | HTTP verb | API version | Extra header |
| --- | --- | --- | --- |
| Create position | POST | 2 | — |
| Amend position | PUT | 2 | — |
| Amend position level | PUT | 2 | — |
| Close position | POST | 1 | `_method: DELETE` |
| Create working order | POST | 2 | — |
| Cancel working order | DELETE | 2 | — |
| Amend working order | PUT | 2 | — |

The strict transport is persistent and pooled, alongside the existing standard
transport. It is constructed with the HTTP client, never per business request.
Rate limiting remains active. Canonical trading endpoints retain their existing
trading budget and primary-key behavior. Opaque encoded endpoint aliases do not
guarantee identical rate classification; use the service methods or canonical IG
paths when that classification matters.

## Authentication and uncertain outcomes

Initial authentication and proactive refresh can precede the business send.
After a mutation has been sent, a 401 returns its existing typed error without
forced refresh or replay. Transport failures, HTTP errors, redirects, and invalid
success payloads likewise cannot trigger a second mutation.

Authentication's shared transport now rejects redirects for login, refresh,
account switching, and logout. This intentionally closes an inherited case in
which a redirected login could forward its password body and IG headers. The
final configured IG base URL must accept those requests directly.

An SDK error does not establish that IG rejected or failed to receive a mutation.
The consuming engine must persist the intent and request identity, mark dispatch
durably, and reconcile an uncertain result before allowing another submission.
A separate caller invocation is a new request. Cancelling a future cannot recall
an order already received by IG. This change supplies no broker-side idempotency
or exactly-once guarantee.

## Deliberate limits

- Caller-owned `make_http_request*` free functions cannot override the supplied
  reqwest transport's redirect/protocol-retry settings. A zero SDK retry count in
  those helpers is insufficient for the complete guarantee; use `HttpClient`.
- An attempt limit is not a deadline. The inherited `RestApiConfig::timeout`
  setting is not applied by the HTTP/Auth builders in this change. Consumers
  needing a deadline must enforce one and still reconcile ambiguous sends.
- Initial login and refresh are not made single-flight. Concurrent callers may
  initiate separate authentication calls before sending their business requests.
- Standard business-read redirects and their inherited cross-origin IG-header
  forwarding behavior are unchanged. The Auth correction does not certify those
  separate paths as safe for arbitrary redirect destinations.
- Existing complete-URL diagnostics are not comprehensively redacted here. This
  is a mutation-replay boundary change, not a whole-SDK security certification.

## Executed validation and safe reproduction

The implementation agent ran 72 selected tests with explicit synthetic
configuration and local listeners: 14 mutation cases, 3 transport/classifier
cases, 10 auth-flow cases, 9 HTTP-read retry cases, and 36 key-pool cases. Each
selected test may cover multiple operations and statuses. Cases include all seven
mutations, v2/OAuth authentication rejection, allowance errors, redirects,
malformed success payloads, received-request connection loss, truncated bodies,
canonical and escaped endpoint aliases, and standard-read regressions.

The HTTP/2 fixture counts actual request HEADERS streams under `REFUSED_STREAM`:
the standard transport sends three, while the strict transport sends one. The
test concerns request replay, not the number of TCP connection attempts.

All-target Clippy with warnings denied passed for REST-only, persistence-only,
streaming-only, and all features. The all-feature release build passed. The
architect separately regenerated README via `make readme` and checked the final
public documentation and prelude surface: formatting, all-feature all-target
Clippy, default-feature docs, and REST-only rustdoc passed with warnings denied.
A doctest command intended to select the new `no_run` example instead ran the
Rust 2024 merged doctest group: 12 passed and 6 ignored. Network and
environment-loading examples were compile-only; the executed examples were
imports, pure helpers, and injected configuration. Detailed commands and
execution logs are saved under the prepared checkout's ignored `target/ci/`
directory. This doctest outcome is recorded separately from the 72 selected
implementation tests.

The complete upstream suite and `make pre-push` were **not** executed: existing
unselected tests invoke environment-loading constructors. Do not substitute an
unfiltered `cargo test` or feature-matrix test target for the audited selectors.
Run the following only with a sanitized process environment, local-only test
configuration, and the resolved offline lockfile used for this handoff:

```sh
cargo test -p ig-client --locked --offline --no-default-features --test unit_tests application::test_mutation_policy::
cargo test -p ig-client --locked --offline --no-default-features --lib application::http::single_attempt_tests::
cargo test -p ig-client --locked --offline --no-default-features --test unit_tests application::test_auth_flow::
cargo test -p ig-client --locked --offline --no-default-features --test unit_tests application::test_http_request::
cargo test -p ig-client --locked --offline --no-default-features --test unit_tests application::test_key_pool::
```

No authenticated IG test, real mutation, release, registry publication, or
ig-engine adoption is claimed. No dependency, CI, Makefile, or package-version
change is part of this branch.
