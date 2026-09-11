# Market catalog enumeration and expiry

Starting with 0.17.0, `MarketService::get_all_markets()` enumerates the account's
catalog through `GET /categories` and `GET /categories/{categoryId}/instruments`.
`get_vec_db_entries()` uses that enumeration. Both methods retain their existing
`Result<Vec<_>, AppError>` signatures.

The low-level `get_market_navigation()` and `get_market_navigation_node()` methods
remain available to explicit callers. High-level catalog enumeration no longer
uses them or falls back to them after a category request fails.

## Scope and request contract

Every category returned for the account is visited, including categories marked
`nonTradeable`. The client does not restrict the catalog to options, poller
categories, particular symbols, market statuses, or OTC-tradeable instruments.
An empty category list is a completed empty account catalog.

Both category endpoints use `Version: 1`. Instrument pages start at
`pageNumber=0`, with `pageSize=500`. The low-level
`get_category_instruments(category_id, page_number, page_size)` remains a
single-page operation; an explicitly supplied page size must be in `1..=1000`.
All requests use the shared HTTP client, existing non-trading rate controls,
finite retry policy, and session refresh handling. Detail enrichment overlaps at
most six requests and remains subject to those same controls.

IG documents the category routes, zero-based pages, and the allowed page-size
range. The references were checked on 2026-09-11. They expose `pageNumber` and
`pageSize` metadata, but no total, next-page marker, or snapshot token.
See the official [categories reference](https://labs.ig.com/reference/categories.html)
and [category instruments reference](https://labs.ig.com/reference/categories-category-id-instruments.html).

## Completion and pagination

A category completes only after a successful response with an empty `instruments`
array and valid metadata. A short nonempty page is followed by another request.
A page containing only instruments already seen elsewhere also requires another
request. Empty-page termination is the client's conservative policy; IG's public
reference does not explicitly specify terminal-page behavior. Its behavior on
the consuming account still needs integration verification.

Before accepting any page, including an empty one, the client checks:

- Metadata exists and its `pageNumber` matches the requested page.
- The effective `pageSize` is in `1..=500`, remains unchanged within the category,
  and is at least the number of returned rows. A stable server-selected size
  below the requested 500 is accepted.
- Every instrument has a nonblank EPIC.
- A nonempty page's set of EPICs has not appeared earlier in the same category.
  Changed prices or a different row order do not disguise a repeated page.

At most 1,000 page requests are allowed per category, counting the empty terminal
page. Page indices `0..=999` are available. If page 999 is nonempty, the client
returns an error instead of reporting a complete catalog. Page counters use
checked arithmetic.

Repeated category codes are visited once. Instruments are deduplicated by EPIC
across all pages and categories; the first listing wins, and its position in the
result is retained. Fields from conflicting later listings are not merged.

`Ok(markets)` means all returned categories completed under this policy. It does
not promise an atomic snapshot: additions or removals while IG serves successive
pages can still affect the result because the API exposes no snapshot mechanism.

## Errors

| Result | Meaning |
| --- | --- |
| `AppError::CatalogRequest { endpoint, source }` | A categories or instruments request failed, including transport, authentication, HTTP, or decoding failures. `endpoint` includes page parameters where applicable; `source` retains the original typed `AppError` and participates in the standard error-source chain. |
| `AppError::CatalogPagination { category_id, page_number, reason }` | Metadata is missing or inconsistent, an identifier is blank, a nonempty page repeats, or the traversal reaches its safety limit. |
| `Ok(Vec::new())` | The account has no returned categories, or every category completed with no instruments. |

Neither public method converts a failed category/page into a successful partial
vector. Callers must preserve the error instead of substituting an empty catalog
or treating it as a signal to remove previously known instruments. A caller
matching an underlying allowance or authentication error now examines the
`CatalogRequest.source` wrapper.

## Field conversion and expiry

`From<CategoryInstrument> for MarketData` explicitly retains every field modeled
by `CategoryInstrument`. `market_status` uses IG's wire spelling. Category `high`
and `low` are session prices, so they populate separate fields; they do not become
`high_limit_price` or `low_limit_price`. Price limits and `update_time_utc` remain
`None` because the category listing does not supply them.

Each database entry preserves its own listing `expiry` string and optional
`expiry_timestamp`. `expiry_timestamp` retains the existing DTO's Unix epoch
milliseconds representation without conversion or inference. This extension is
present in an existing repository payload fixture but is omitted from the public
category reference; this release does not establish a new guarantee about its
availability on every category or account.

Expiry strings are kept verbatim, including values such as `-`, `DFB`, and
month/year labels. The client no longer groups by symbol to select a
representative EPIC or copies that instrument's date to its peers.

Only an entry whose expiry text is blank is eligible for details enrichment:

1. Fetch `GET /markets/{epic}` with `Version: 3` for that entry's EPIC.
2. Accept the details only when `instrument.epic` equals the requested EPIC.
3. Copy a nonblank `instrument.expiry` into that entry alone.
4. If supplied, retain `expiryDetails.lastDealingDate` separately as
   `last_dealing_date`. The listing's `expiry_timestamp` stays unchanged.

A details failure or EPIC mismatch emits a warning and retains that entry's
listing values. The catalog can therefore be complete while optional expiry
enrichment remains unavailable. A blank expiry stays unknown; no other
instrument's value is substituted. Entries that already have expiry text do not
request details, so their `last_dealing_date` remains `None`.

IG describes `instrument.expiry` and `expiryDetails.lastDealingDate` as separate
fields. The client does not treat them as equivalent or add minutes to one to
produce the other. See the official [market details reference](https://labs.ig.com/reference/markets-epic.html).
This change affects returned client data only; it performs no historical rewrite
or database migration.

## Migration from 0.16.5

Use the 0.17 release line once publication is confirmed. Ordinary method calls
remain unchanged. The minor-version change accounts for source compatibility
changes to public structs and exhaustive error matches:

| Public type | Added fields or variants |
| --- | --- |
| `MarketData` | `lot_size`, `otc_tradeable`, `delay_time`, `high`, `low`, `scaling_factor`, `expiry_timestamp`, `underlying_name`, `popularity`, `market_type`, `market_subtype` |
| `DBEntryResponse` | `expiry_timestamp`, `last_dealing_date` |
| `AppError` | `CatalogRequest`, `CatalogPagination` |

Update struct literals with explicit values for the new optional fields, or keep
all existing values and add `..Default::default()`. `MarketData` now implements
`Default` for this purpose. Use defaults only where the additional values are
unknown; do not discard known data. Destructuring can use `..` for unused fields.
Update exhaustive `AppError` matches and any serialized-output schemas.

New fields are optional, accept their absence when deserializing, and are omitted
when serializing `None`. `MarketData` uses the corresponding IG camelCase field
names; `DBEntryResponse` retains its existing snake_case serialization convention.

`DBEntryResponse.symbol` still comes from the third dot-separated EPIC segment.
That existing convention and name-based option parsing have not been validated
against DATA-ENGINE's real catalog by these changes. The actual symbol and chain
mapping remains part of the consumer's integration verification.

## Verification and DATA-ENGINE adoption

The public-client regressions in
[`test_market_catalog.rs`](../tests/unit/application/test_market_catalog.rs) use
local HTTP mocks. Their categories, instruments, dates, and failures are synthetic
fixtures, not claims about observed production incidents. They exercise both
public methods, pagination failures, duplicate handling, and individual expiries.
DTO conversion round-trips also reuse the existing category payload fixture.

Official documentation was checked; this change has not been exercised against
an authenticated IG account. Compilation and fixture checks do not establish
successful enumeration or symbol mapping against IG. The full mock HTTP suite
must also pass in an environment that permits local test-server sockets; it
could not run to completion in the restricted development environment.

DATA-ENGINE adoption requires the following steps:

1. Confirm that `ig-client` 0.17.0 has been published to the dependency registry
   before updating the dependency and lockfile. A local version bump or package
   archive is not publication confirmation.
2. Adapt struct literals and serialized schemas for the added optional fields,
   and update exhaustive error matches as described above. Preserve
   `CatalogRequest.source` when classifying request failures.
3. Run DATA-ENGINE's integration suite against its actual IG account. Check the
   complete category/page traversal and real symbol/chain mapping; a correctly
   propagated error alone does not establish that the mapping works.
4. Check distinct instruments of the same symbol retain their individual expiry
   text and available timestamps, including failed enrichment, and keep
   `last_dealing_date` separate. Apply the change to newly returned client data
   without rewriting historical records.

Report fixture results separately from authenticated IG and consumer-integration
results when coordinating adoption.
