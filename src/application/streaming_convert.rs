/******************************************************************************
   Author: Joaquín Béjar García
   Email: jb@taunais.com
   Date: 20/10/25
******************************************************************************/

//! Adapters from Lightstreamer `ItemUpdate` to the presentation-layer DTOs.
//!
//! The presentation DTOs stay transport-agnostic: they expose pure
//! `from_fields(..)` constructors that take plain field maps. This module is the
//! only place that depends on `lightstreamer_rs`; it reads an `ItemUpdate` and
//! feeds the extracted metadata / field maps into those pure constructors.
//!
//! Both a `Result`-returning function (for callers that want to observe parse
//! failures) and the `From<&ItemUpdate>` conversions (which degrade to a default
//! on failure, preserving the previous streaming behaviour) are provided per
//! type.

use crate::presentation::account::AccountData;
use crate::presentation::chart::ChartData;
use crate::presentation::market::{MarketFields, PresentationMarketData};
use crate::presentation::price::PriceData;
use crate::presentation::trade::TradeData;
use lightstreamer_rs::subscription::ItemUpdate;
use std::collections::HashMap;

/// Converts Lightstreamer's `changed_fields` (`HashMap<String, String>`) into the
/// `HashMap<String, Option<String>>` shape the pure field parsers consume, so a
/// changed value is treated identically to a present full-snapshot value.
#[must_use]
fn changed_fields_as_options(changed: &HashMap<String, String>) -> HashMap<String, Option<String>> {
    changed
        .iter()
        .map(|(key, value)| (key.clone(), Some(value.clone())))
        .collect()
}

/// Converts a Lightstreamer `ItemUpdate` into a [`PriceData`].
///
/// # Errors
/// Returns an error string when a field fails to parse (e.g. a non-numeric
/// price or an unknown dealing flag).
#[must_use = "the parse result must be handled"]
pub fn price_data_from_item_update(item_update: &ItemUpdate) -> Result<PriceData, String> {
    let changed_fields = changed_fields_as_options(&item_update.changed_fields);
    PriceData::from_fields(
        item_update.item_name.as_deref(),
        item_update.item_pos,
        item_update.is_snapshot,
        &item_update.fields,
        &changed_fields,
    )
}

impl From<&ItemUpdate> for PriceData {
    fn from(item_update: &ItemUpdate) -> Self {
        price_data_from_item_update(item_update).unwrap_or_else(|e| {
            tracing::warn!(error = %e, "failed to convert ItemUpdate to PriceData, returning default");
            PriceData::default()
        })
    }
}

/// Converts a Lightstreamer `ItemUpdate` into a [`PresentationMarketData`].
///
/// # Errors
/// Returns an error string when a field fails to parse (e.g. an unknown market
/// state or an invalid `MARKET_DELAY` value).
#[must_use = "the parse result must be handled"]
pub fn market_data_from_item_update(
    item_update: &ItemUpdate,
) -> Result<PresentationMarketData, String> {
    let changed_fields = changed_fields_as_options(&item_update.changed_fields);
    PresentationMarketData::from_fields(
        item_update.item_name.as_deref(),
        item_update.item_pos,
        item_update.is_snapshot,
        &item_update.fields,
        &changed_fields,
    )
}

impl From<&ItemUpdate> for PresentationMarketData {
    fn from(item_update: &ItemUpdate) -> Self {
        market_data_from_item_update(item_update).unwrap_or_else(|_| PresentationMarketData {
            item_name: String::new(),
            item_pos: 0,
            fields: MarketFields::default(),
            changed_fields: MarketFields::default(),
            is_snapshot: false,
        })
    }
}

/// Converts a Lightstreamer `ItemUpdate` into a [`ChartData`].
///
/// # Errors
/// Returns an error string when a field fails to parse (e.g. a non-numeric
/// candle value).
#[must_use = "the parse result must be handled"]
pub fn chart_data_from_item_update(item_update: &ItemUpdate) -> Result<ChartData, String> {
    let changed_fields = changed_fields_as_options(&item_update.changed_fields);
    ChartData::from_fields(
        item_update.item_name.as_deref(),
        item_update.item_pos,
        item_update.is_snapshot,
        &item_update.fields,
        &changed_fields,
    )
}

impl From<&ItemUpdate> for ChartData {
    fn from(item_update: &ItemUpdate) -> Self {
        chart_data_from_item_update(item_update).unwrap_or_default()
    }
}

/// Converts a Lightstreamer `ItemUpdate` into a [`TradeData`].
///
/// # Errors
/// Returns an error string when the embedded OPU / WOU JSON payload fails to
/// parse.
#[must_use = "the parse result must be handled"]
pub fn trade_data_from_item_update(item_update: &ItemUpdate) -> Result<TradeData, String> {
    let changed_fields = changed_fields_as_options(&item_update.changed_fields);
    TradeData::from_fields(
        item_update.item_name.as_deref(),
        item_update.item_pos,
        item_update.is_snapshot,
        &item_update.fields,
        &changed_fields,
    )
}

impl From<&ItemUpdate> for TradeData {
    fn from(item_update: &ItemUpdate) -> Self {
        trade_data_from_item_update(item_update).unwrap_or_default()
    }
}

/// Converts a Lightstreamer `ItemUpdate` into an [`AccountData`].
///
/// # Errors
/// Returns an error string when a field fails to parse (e.g. a non-numeric P&L
/// or margin value).
#[must_use = "the parse result must be handled"]
pub fn account_data_from_item_update(item_update: &ItemUpdate) -> Result<AccountData, String> {
    let changed_fields = changed_fields_as_options(&item_update.changed_fields);
    AccountData::from_fields(
        item_update.item_name.as_deref(),
        item_update.item_pos,
        item_update.is_snapshot,
        &item_update.fields,
        &changed_fields,
    )
}

impl From<&ItemUpdate> for AccountData {
    fn from(item_update: &ItemUpdate) -> Self {
        account_data_from_item_update(item_update).unwrap_or_else(|_| AccountData::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presentation::order::{Direction, OrderType, Status, TimeInForce};

    /// Builds an `ItemUpdate` carrying a single TRADE field, mirroring how the
    /// Lightstreamer TRADE subscription delivers OPU / WOU / CONFIRMS payloads.
    ///
    /// `item_name`, `item_pos` and `is_snapshot` are set to distinctive,
    /// obviously-synthetic values so tests can assert whether they survive the
    /// parse (they are dropped on the silent-default path — see the malformed
    /// test).
    fn trade_item_update(field_key: &str, field_value: &str) -> ItemUpdate {
        let mut fields = HashMap::new();
        fields.insert(field_key.to_string(), Some(field_value.to_string()));
        ItemUpdate {
            item_name: Some("TRADE:SYNTHETIC".to_string()),
            item_pos: 2,
            fields,
            changed_fields: HashMap::new(),
            is_snapshot: true,
        }
    }

    // Realistic, sanitized OPU payload. Field names mirror IG's Open Position
    // Update stream (camelCase); the deal ids are obviously synthetic and carry
    // no secret material.
    const SANITIZED_OPU_JSON: &str = r#"{
        "dealReference": "REF_SYNTHETIC_0001",
        "dealId": "DIAAAAF00SYNTH01",
        "dealIdOrigin": "DIAAAAF00SYNTH01",
        "direction": "BUY",
        "epic": "IX.D.DAX.DAILY.IP",
        "status": "OPEN",
        "dealStatus": "ACCEPTED",
        "level": "18000.5",
        "size": "1.0",
        "currency": "EUR",
        "timestamp": "2024-01-15T10:30:00.000",
        "channel": "PublicRestOTC",
        "expiry": "DFB"
    }"#;

    // Realistic, sanitized WOU payload. Field names mirror IG's Working Order
    // Update stream (camelCase); the deal ids are obviously synthetic.
    const SANITIZED_WOU_JSON: &str = r#"{
        "dealReference": "REF_SYNTHETIC_0002",
        "dealId": "DIAAAAF00SYNTH02",
        "direction": "SELL",
        "epic": "CS.D.EURUSD.CFD.IP",
        "status": "OPEN",
        "dealStatus": "ACCEPTED",
        "level": "1.1000",
        "size": "10000",
        "currency": "USD",
        "timestamp": "2024-01-15T10:31:00.000",
        "channel": "PublicRestOTC",
        "expiry": "-",
        "stopDistance": "10.5",
        "limitDistance": "20.0",
        "guaranteedStop": false,
        "orderType": "LIMIT",
        "timeInForce": "GOOD_TILL_CANCELLED",
        "goodTillDate": ""
    }"#;

    #[test]
    fn test_trade_data_from_item_update_parses_open_position_update() {
        let item = trade_item_update("OPU", SANITIZED_OPU_JSON);

        let result = trade_data_from_item_update(&item);
        assert!(result.is_ok(), "OPU payload should parse: {result:?}");
        let data = result.expect("OPU parse checked ok above");

        // Update metadata is preserved on the success path.
        assert_eq!(data.item_name, "TRADE:SYNTHETIC");
        assert_eq!(data.item_pos, 2);
        assert!(data.is_snapshot);

        let opu = data.fields.opu.expect("OPU should be populated");
        assert_eq!(opu.deal_reference.as_deref(), Some("REF_SYNTHETIC_0001"));
        assert_eq!(opu.deal_id.as_deref(), Some("DIAAAAF00SYNTH01"));
        assert_eq!(opu.deal_id_origin.as_deref(), Some("DIAAAAF00SYNTH01"));
        assert_eq!(opu.direction, Some(Direction::Buy));
        assert_eq!(opu.epic.as_deref(), Some("IX.D.DAX.DAILY.IP"));
        assert_eq!(opu.status, Some(Status::Open));
        assert_eq!(opu.deal_status, Some(Status::Accepted));
        assert_eq!(opu.level, Some(18000.5));
        assert_eq!(opu.size, Some(1.0));
        assert_eq!(opu.currency.as_deref(), Some("EUR"));
        assert_eq!(opu.expiry.as_deref(), Some("DFB"));

        // No WOU or CONFIRMS present in this update.
        assert!(data.fields.wou.is_none());
        assert!(data.fields.confirms.is_none());
    }

    #[test]
    fn test_trade_data_from_item_update_parses_working_order_update() {
        let item = trade_item_update("WOU", SANITIZED_WOU_JSON);

        let result = trade_data_from_item_update(&item);
        assert!(result.is_ok(), "WOU payload should parse: {result:?}");
        let data = result.expect("WOU parse checked ok above");

        let wou = data.fields.wou.expect("WOU should be populated");
        assert_eq!(wou.deal_reference.as_deref(), Some("REF_SYNTHETIC_0002"));
        assert_eq!(wou.deal_id.as_deref(), Some("DIAAAAF00SYNTH02"));
        assert_eq!(wou.direction, Some(Direction::Sell));
        assert_eq!(wou.epic.as_deref(), Some("CS.D.EURUSD.CFD.IP"));
        assert_eq!(wou.status, Some(Status::Open));
        assert_eq!(wou.deal_status, Some(Status::Accepted));
        assert_eq!(wou.level, Some(1.1000));
        assert_eq!(wou.size, Some(10000.0));
        assert_eq!(wou.currency.as_deref(), Some("USD"));
        assert_eq!(wou.stop_distance, Some(10.5));
        assert_eq!(wou.limit_distance, Some(20.0));
        assert_eq!(wou.guaranteed_stop, Some(false));
        assert_eq!(wou.order_type, Some(OrderType::Limit));
        assert_eq!(wou.time_in_force, Some(TimeInForce::GoodTillCancelled));
        // Empty string good-till-date degrades to None.
        assert!(wou.good_till_date.is_none());

        // No OPU or CONFIRMS present in this update.
        assert!(data.fields.opu.is_none());
        assert!(data.fields.confirms.is_none());
    }

    #[test]
    fn test_trade_data_from_item_update_malformed_opu_is_error_on_direct_path() {
        let item = trade_item_update("OPU", "{ this is not valid json ");

        // The direct parse path surfaces the failure as an Err.
        let result = trade_data_from_item_update(&item);
        assert!(
            result.is_err(),
            "malformed OPU JSON must surface an error on the direct parse path"
        );
    }

    #[test]
    fn test_trade_data_from_impl_swallows_malformed_opu_into_default() {
        // KNOWN GAP (pinned, do not "fix" here): `From<&ItemUpdate>` funnels any
        // parse failure through `.unwrap_or_default()`, so a malformed TRADE
        // payload is silently swallowed into `TradeData::default()` instead of
        // being surfaced. This test PINS that current behavior so a future change
        // to make the swallow deliberate (e.g. logging, or propagating the error)
        // is a conscious, reviewed decision rather than an accidental regression.
        //
        // TODO(streaming): decide whether `From<&ItemUpdate>` should log the
        // dropped payload at WARN or expose the parse error instead of defaulting.
        let item = trade_item_update("OPU", "{ this is not valid json ");

        let data = TradeData::from(&item);

        // Everything degrades to the Default, including the update metadata that
        // was present on the source ItemUpdate (item_name / item_pos / snapshot).
        assert!(
            data.item_name.is_empty(),
            "silent-default path discards item_name"
        );
        assert_eq!(data.item_pos, 0, "silent-default path discards item_pos");
        assert!(
            !data.is_snapshot,
            "silent-default path discards is_snapshot"
        );
        assert!(data.fields.opu.is_none());
        assert!(data.fields.wou.is_none());
        assert!(data.fields.confirms.is_none());
    }

    #[test]
    fn test_price_data_from_item_update_defaults_on_unknown_dealing_flag() {
        // An unknown dealing flag makes the pure parser fail; the `From` impl
        // degrades to `PriceData::default()` while the direct fn surfaces the Err.
        let mut fields = HashMap::new();
        fields.insert("DLG_FLAG".to_string(), Some("NOT_A_FLAG".to_string()));
        let item = ItemUpdate {
            item_name: Some("PRICE:CS.D.EURUSD.MINI.IP".to_string()),
            item_pos: 1,
            fields,
            changed_fields: HashMap::new(),
            is_snapshot: true,
        };

        assert!(price_data_from_item_update(&item).is_err());
        // The `From` impl degrades to the default: metadata from the source
        // update is discarded (matching the pre-refactor silent-default path).
        let data = PriceData::from(&item);
        assert!(data.item_name.is_empty());
        assert_eq!(data.item_pos, 0);
        assert!(!data.is_snapshot);
        assert!(data.fields.dealing_flag.is_none());
    }
}
