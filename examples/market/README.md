# Market examples

Run these commands from the repository root after configuring the IG credentials
used by `Client::try_new()`. The default REST endpoint is the IG demo gateway;
`IG_REST_BASE_URL` selects another endpoint explicitly. Live commands require a
working account/session and consume the normal shared request allowances.
Compilation and synthetic tests do not establish live account availability.

## Catalogue enumeration and exports

```sh
cargo run -p examples_market --bin categories
cargo run -p examples_market --bin get_all_markets
cargo run -p examples_market --bin market_hierarchy
cargo run -p examples_market --bin market_table
cargo run -p examples_market --bin vec_db_entries_table
```

`get_all_markets` traverses every account category, including non-tradeable
categories, and deduplicates EPICs. Pages start at zero; every category must reach
a validated empty page. Request failures, inconsistent metadata, repeated pages,
and the page safety bound return an error instead of a complete result.

The historical binary name `market_hierarchy` is retained, but it now saves the
complete **flat** `Vec<MarketData>` to `Data/market_catalog.json`. It does not use
`marketnavigation` or construct an IG navigation tree. `market_table` saves
`Data/market_table.json`, with parsed name fields and each full market DTO nested
under `market`. Both exports occur only after enumeration succeeds.

Each EPIC retains its own `expiry` and optional `expiryTimestamp`. The database
entry table uses `get_vec_db_entries()`: only blank expiry text triggers an
individual detail lookup. Last dealing dates remain separate from expiry, and
failed or mismatched enrichment preserves the listed values. An undated value
such as `-` or `DFB` remains a value rather than an inferred timestamp.

## Inspecting one category page

Use an exact category code from the `categories` output:

```sh
cargo run -p examples_market --bin category_instruments -- CATEGORY_ID 0 150
```

The page number defaults to `0`; page size defaults to `150` and must be in
`1..=1000`. Invalid arguments and categories not enabled for the account are
errors. This command retrieves exactly one page and saves the entire response,
including metadata, to
`Data/category_CATEGORY_ID_page_PAGE_size_SIZE.json` (filename characters are
sanitized). It does not declare the category or the whole catalogue complete.

## Explicit detail requests

```sh
cargo run -p examples_market --bin market_details -- 'EPIC_ONE,EPIC_TWO'
```

Supply distinct EPICs from the catalogue. The example requests batches of up to
25; an HTTP batch failure falls back to individual requests. Every requested
EPIC must appear exactly once. Responses are associated by their own instrument
EPIC, so response reordering cannot assign an expiry to another instrument.
Missing, duplicate, unexpected, or failed detail responses abort the export.
On success, `Data/market_details.json` contains the full detail DTO array, with
`instrument.expiry` and `instrument.expiryDetails.lastDealingDate` kept separate.

## Filtered storage

```sh
cargo run -p examples_market --bin market_hierarchy
cargo run -p examples_market --bin filtered_market_storage -- Data/market_catalog.json
```

`filtered_market_storage` defaults to that same catalogue path and requires
PostgreSQL configuration. The existing storage adapter accepts EPICs containing
exactly four dots. The example's name-to-symbol mapping labels accepted records;
it does not exclude unmatched names, which receive `UNKNOWN`. It adapts the input
instruments to leaf `MarketNode` records for the existing storage API; these leaves
do not represent an inferred IG category tree. The input is a flat catalogue
array, replacing the old
`market_hierarchy_backup.json` prerequisite. This example writes to the configured
database, so run it only when that storage operation is intended.

All market JSON exporters create `Data/` relative to the current working
directory. Historical-price examples still need an EPIC and date range supported
by the configured account's historical-data allowance.

## Offline regression checks

```sh
cargo test -p examples_market --bin category_instruments --bin market_details --bin vec_db_entries_table
```

These tests use synthetic in-memory fixtures. They cover invalid page arguments,
reordered/missing/duplicate/unexpected detail identities, individual expiry
association, and Unicode-safe display truncation without contacting IG or a
database. They do not execute the example `main` functions.
