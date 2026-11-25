# Changelog

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
