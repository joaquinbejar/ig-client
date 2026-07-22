/******************************************************************************
   Author: Joaquín Béjar García
   Email: jb@taunais.com
   Date: 20/10/25
******************************************************************************/

//! Adapters from a Lightstreamer update to the presentation-layer DTOs.
//!
//! The presentation DTOs stay transport-agnostic: they expose pure
//! `from_fields(..)` constructors that take plain field maps. This module is the
//! only place that depends on `lightstreamer_rs`; it reads an
//! [`ItemUpdate`](lightstreamer_rs::ItemUpdate) and
//! feeds the extracted metadata / field maps into those pure constructors.
//!
//! # The seam
//!
//! [`ItemUpdate`](lightstreamer_rs::ItemUpdate) is the streaming crate's own
//! type: it borrows from the subscription schema, has no public constructor, and
//! models a field value as [`FieldValue`](lightstreamer_rs::FieldValue) rather
//! than a string. [`StreamingUpdate`](crate::application::streaming_convert::StreamingUpdate) is this crate's
//! owned, constructible mirror of it, and every conversion below goes through
//! it. That keeps the DTO parsers testable without a live session, and keeps
//! the null-versus-empty distinction explicit at exactly one place:
//! `FieldValue::Null` becomes `None`, `FieldValue::Text(s)` becomes
//! `Some(s)` — including `Some("")` for a field the server deliberately sent
//! empty.
//!
//! Both a `Result`-returning function (for callers that want to observe parse
//! failures) and the `From` conversions (which degrade to a default on failure,
//! preserving the previous streaming behaviour) are provided per type.

use crate::error::AppError;
use crate::presentation::account::AccountData;
use crate::presentation::chart::ChartData;
use crate::presentation::market::{MarketFields, PresentationMarketData};
use crate::presentation::price::PriceData;
use crate::presentation::trade::TradeData;
use lightstreamer_rs::{FieldValue, ItemUpdate};
use pretty_simple_display::{DebugPretty, DisplaySimple};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// One Lightstreamer item update, owned and free of borrowed schema state.
///
/// This is the boundary type between `lightstreamer-rs` and this crate's
/// presentation DTOs. It is deliberately constructible by hand so that the
/// field parsers can be exercised from captured IG payloads without a live
/// session — the streaming crate's own [`ItemUpdate`] cannot be built outside
/// that crate.
///
/// A `None` field value means the server said the field has *no* value
/// (`FieldValue::Null`); `Some("")` means it sent an empty one. The two are
/// different answers and are kept apart.
#[derive(DebugPretty, DisplaySimple, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct StreamingUpdate {
    /// The item name the subscription declared for this position, when it
    /// declared one. `None` for a server-resolved item group, where the
    /// protocol never transmits names.
    pub item_name: Option<String>,
    /// The item's 1-based position within the subscription.
    pub item_pos: usize,
    /// Whether this update carries snapshot content rather than a live change.
    pub is_snapshot: bool,
    /// The complete state of the item after this update, keyed by field name.
    pub fields: HashMap<String, Option<String>>,
    /// Only the fields this update actually carried a value for.
    pub changed_fields: HashMap<String, Option<String>>,
}

/// Converts a [`FieldValue`] into the `Option<String>` the pure field parsers
/// consume: null becomes absent, text (empty or not) becomes present.
#[must_use]
#[inline]
fn field_value_as_option(value: FieldValue<'_>) -> Option<String> {
    match value {
        FieldValue::Null => None,
        FieldValue::Text(text) => Some(text.to_owned()),
    }
}

impl From<&ItemUpdate> for StreamingUpdate {
    fn from(update: &ItemUpdate) -> Self {
        let fields = update
            .fields()
            .map(|field| {
                (
                    field.name().to_owned(),
                    field_value_as_option(field.value()),
                )
            })
            .collect();
        let changed_fields = update
            .changed_fields()
            .map(|field| {
                (
                    field.name().to_owned(),
                    field_value_as_option(field.value()),
                )
            })
            .collect();

        Self {
            item_name: update.declared_item_name().map(str::to_owned),
            item_pos: update.item_index(),
            is_snapshot: update.is_snapshot(),
            fields,
            changed_fields,
        }
    }
}

/// Converts a streaming update into a [`PriceData`].
///
/// # Errors
/// Returns [`AppError::Deserialization`] when a field fails to parse (e.g. a
/// non-numeric price or an unknown dealing flag). The message names the
/// offending field and value.
#[must_use = "the parse result must be handled"]
pub fn price_data_from_item_update(update: &StreamingUpdate) -> Result<PriceData, AppError> {
    PriceData::from_fields(
        update.item_name.as_deref(),
        update.item_pos,
        update.is_snapshot,
        &update.fields,
        &update.changed_fields,
    )
    .map_err(AppError::Deserialization)
}

impl From<&StreamingUpdate> for PriceData {
    fn from(update: &StreamingUpdate) -> Self {
        price_data_from_item_update(update).unwrap_or_else(|e| {
            tracing::warn!(error = %e, "failed to convert streaming update to PriceData, returning default");
            PriceData::default()
        })
    }
}

impl From<&ItemUpdate> for PriceData {
    fn from(update: &ItemUpdate) -> Self {
        Self::from(&StreamingUpdate::from(update))
    }
}

/// Converts a streaming update into a [`PresentationMarketData`].
///
/// # Errors
/// Returns [`AppError::Deserialization`] when a field fails to parse (e.g. an
/// unknown market state or an invalid `MARKET_DELAY` value).
#[must_use = "the parse result must be handled"]
pub fn market_data_from_item_update(
    update: &StreamingUpdate,
) -> Result<PresentationMarketData, AppError> {
    PresentationMarketData::from_fields(
        update.item_name.as_deref(),
        update.item_pos,
        update.is_snapshot,
        &update.fields,
        &update.changed_fields,
    )
    .map_err(AppError::Deserialization)
}

impl From<&StreamingUpdate> for PresentationMarketData {
    fn from(update: &StreamingUpdate) -> Self {
        market_data_from_item_update(update).unwrap_or_else(|_| PresentationMarketData {
            item_name: String::new(),
            item_pos: 0,
            fields: MarketFields::default(),
            changed_fields: MarketFields::default(),
            is_snapshot: false,
        })
    }
}

impl From<&ItemUpdate> for PresentationMarketData {
    fn from(update: &ItemUpdate) -> Self {
        Self::from(&StreamingUpdate::from(update))
    }
}

/// Converts a streaming update into a [`ChartData`].
///
/// # Errors
/// Returns [`AppError::Deserialization`] when a field fails to parse (e.g. a
/// non-numeric candle value).
#[must_use = "the parse result must be handled"]
pub fn chart_data_from_item_update(update: &StreamingUpdate) -> Result<ChartData, AppError> {
    ChartData::from_fields(
        update.item_name.as_deref(),
        update.item_pos,
        update.is_snapshot,
        &update.fields,
        &update.changed_fields,
    )
    .map_err(AppError::Deserialization)
}

impl From<&StreamingUpdate> for ChartData {
    fn from(update: &StreamingUpdate) -> Self {
        chart_data_from_item_update(update).unwrap_or_default()
    }
}

impl From<&ItemUpdate> for ChartData {
    fn from(update: &ItemUpdate) -> Self {
        Self::from(&StreamingUpdate::from(update))
    }
}

/// Converts a streaming update into a [`TradeData`].
///
/// # Errors
/// Returns [`AppError::Deserialization`] when the embedded OPU / WOU JSON
/// payload fails to parse.
#[must_use = "the parse result must be handled"]
pub fn trade_data_from_item_update(update: &StreamingUpdate) -> Result<TradeData, AppError> {
    TradeData::from_fields(
        update.item_name.as_deref(),
        update.item_pos,
        update.is_snapshot,
        &update.fields,
        &update.changed_fields,
    )
    .map_err(AppError::Deserialization)
}

impl From<&StreamingUpdate> for TradeData {
    fn from(update: &StreamingUpdate) -> Self {
        trade_data_from_item_update(update).unwrap_or_default()
    }
}

impl From<&ItemUpdate> for TradeData {
    fn from(update: &ItemUpdate) -> Self {
        Self::from(&StreamingUpdate::from(update))
    }
}

/// Converts a streaming update into an [`AccountData`].
///
/// # Errors
/// Returns [`AppError::Deserialization`] when a field fails to parse (e.g. a
/// non-numeric P&L or margin value).
#[must_use = "the parse result must be handled"]
pub fn account_data_from_item_update(update: &StreamingUpdate) -> Result<AccountData, AppError> {
    AccountData::from_fields(
        update.item_name.as_deref(),
        update.item_pos,
        update.is_snapshot,
        &update.fields,
        &update.changed_fields,
    )
    .map_err(AppError::Deserialization)
}

impl From<&StreamingUpdate> for AccountData {
    fn from(update: &StreamingUpdate) -> Self {
        account_data_from_item_update(update).unwrap_or_else(|_| AccountData::default())
    }
}

impl From<&ItemUpdate> for AccountData {
    fn from(update: &ItemUpdate) -> Self {
        Self::from(&StreamingUpdate::from(update))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presentation::order::{Direction, OrderType, Status, TimeInForce};

    /// Builds an update carrying a single TRADE field, mirroring how the
    /// Lightstreamer TRADE subscription delivers OPU / WOU / CONFIRMS payloads.
    ///
    /// `item_name`, `item_pos` and `is_snapshot` are set to distinctive,
    /// obviously-synthetic values so tests can assert whether they survive the
    /// parse (they are dropped on the silent-default path — see the malformed
    /// test).
    fn trade_item_update(field_key: &str, field_value: &str) -> StreamingUpdate {
        let mut fields = HashMap::new();
        fields.insert(field_key.to_string(), Some(field_value.to_string()));
        StreamingUpdate {
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
        let item = StreamingUpdate {
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

    // --- The seam type itself ----------------------------------------------

    #[test]
    fn test_field_value_null_and_empty_text_are_distinct() {
        // The whole reason `StreamingUpdate` stores `Option<String>` rather
        // than `String`: TLCP distinguishes "no value" from "the empty string",
        // and the seam must not flatten one into the other.
        assert_eq!(field_value_as_option(FieldValue::Null), None);
        assert_eq!(
            field_value_as_option(FieldValue::Text("")),
            Some(String::new())
        );
        assert_eq!(
            field_value_as_option(FieldValue::Text("1.2345")),
            Some("1.2345".to_string())
        );
    }

    #[test]
    fn test_streaming_update_serde_round_trip() {
        let mut fields = HashMap::new();
        fields.insert("BID".to_string(), Some("18000.5".to_string()));
        // A null field and an empty-text field must both survive the round trip
        // as themselves.
        fields.insert("OFFER".to_string(), None);
        fields.insert("MARKET_STATE".to_string(), Some(String::new()));

        let mut changed_fields = HashMap::new();
        changed_fields.insert("BID".to_string(), Some("18000.5".to_string()));

        let update = StreamingUpdate {
            item_name: Some("MARKET:IX.D.DAX.DAILY.IP".to_string()),
            item_pos: 3,
            is_snapshot: true,
            fields,
            changed_fields,
        };

        let json = serde_json::to_string(&update).expect("StreamingUpdate should serialize");
        let decoded: StreamingUpdate =
            serde_json::from_str(&json).expect("StreamingUpdate should deserialize");

        assert_eq!(decoded, update);
        assert_eq!(decoded.fields.get("OFFER"), Some(&None));
        assert_eq!(
            decoded.fields.get("MARKET_STATE"),
            Some(&Some(String::new()))
        );
    }
}
