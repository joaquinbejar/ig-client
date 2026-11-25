# Changelog

## [0.20.0] - 2026-11-25

### Changed (breaking)

- `PositionDetails::contract_size` is now `Option<f64>`.
- `PositionDetails::created_date_utc` is now `Option<String>`.
- `ExpiryDetails::last_dealing_date` is now `Option<String>`.

A missing or null `contractSize`, `createdDateUTC` or `lastDealingDate` failed
deserialization of the whole positions or market details response. They now
decode as `None`. Adding two positions keeps `contract_size` only when both
sides report it.

## [0.19.0] - 2026-11-25

### Changed (breaking)

- `CategoryInstrument::instrument_type` is now `Option<InstrumentType>`.
- `CategoryInstrument::otc_tradeable` is now `Option<bool>`.
- `CategoryInstrument::market_status` is now `Option<CategoryMarketStatus>`.

IG UK demo `GET /categories/{id}/instruments` rows (for example `OPTIONS`) omit
these fields, so decoding the whole page failed. Converting into `MarketData`
maps a missing type to `InstrumentType::Unknown` and a missing status to
`"UNKNOWN"`.

### Added

- `CategoryInstrument::reference_epic` (`referenceEpic`).
- `CategoryInstrument::maturity_type` (`maturityType`, e.g. `DAILY`, `WEEKLY`).
