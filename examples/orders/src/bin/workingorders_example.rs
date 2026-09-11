//! Create and cancel one working order using its confirmed identity.
//!
//! Run with `cargo run -p examples_orders --bin workingorders_example -- EPIC LIMIT_LEVEL SIZE`.
//! This submits a buy limit order to the configured account; it can execute before cancellation.

use ig_client::model::responses::DealStatus;
use ig_client::prelude::*;
use tracing::info;

const CONFIRMATION_RETRIES: u64 = 3;
const CONFIRMATION_DELAY_MS: u64 = 1_000;
const CANCELLATION_CHECKS: usize = 3;

#[tokio::main]
async fn main() -> Result<(), AppError> {
    setup_logger();
    let mut args = std::env::args().skip(1);
    let usage =
        || AppError::InvalidInput("Usage: workingorders_example EPIC LIMIT_LEVEL SIZE".to_string());
    let epic = args
        .next()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(usage)?;
    let level = args
        .next()
        .ok_or_else(usage)?
        .parse::<f64>()
        .map_err(|_| usage())?;
    let size = args
        .next()
        .ok_or_else(usage)?
        .parse::<f64>()
        .map_err(|_| usage())?;
    if args.next().is_some() || !level.is_finite() || !size.is_finite() || size <= 0.0 {
        return Err(usage());
    }

    let client = Client::try_new()?;
    let details = client.get_market_details(&epic).await?;
    if details.instrument.epic != epic {
        return Err(AppError::InvalidInput(
            "Market details returned a different EPIC".to_string(),
        ));
    }
    let currency = details
        .instrument
        .currencies
        .as_ref()
        .and_then(|currencies| currencies.first())
        .filter(|currency| !currency.code.trim().is_empty())
        .ok_or_else(|| AppError::InvalidInput("Market has no available currency".to_string()))?
        .code
        .clone();
    let request = CreateWorkingOrderRequest::limit(
        epic.clone(),
        Direction::Buy,
        size,
        level,
        currency,
        details.instrument.expiry,
    )
    .expires_tomorrow();

    let submitted = client.create_working_order(&request).await?;
    info!(deal_reference = %submitted.deal_reference, "Working order submitted; awaiting confirmation");
    // This REST-only example has no streaming confirmation subscription.
    let confirmation = client
        .get_order_confirmation_w_retry(
            &submitted.deal_reference,
            CONFIRMATION_RETRIES,
            CONFIRMATION_DELAY_MS,
        )
        .await?;
    let orders = client.get_working_orders().await?;
    let deal_id = order_to_delete(&confirmation, &submitted.deal_reference, &epic, &orders)?;

    info!(
        deal_id,
        "Cancelling the working order created by this example"
    );
    client.delete_working_order(deal_id).await?;
    // The public delete method returns (), so it provides no deletion reference
    // to confirm. Check only that this order leaves the working-order list;
    // absence alone cannot distinguish cancellation from execution or expiry.
    for _ in 0..CANCELLATION_CHECKS {
        tokio::time::sleep(std::time::Duration::from_millis(CONFIRMATION_DELAY_MS)).await;
        let remaining = client.get_working_orders().await?;
        if remaining
            .working_orders
            .iter()
            .all(|order| order.working_order_data.deal_id != deal_id)
        {
            info!(
                deal_id,
                "Cancellation submitted; created order is no longer listed as working"
            );
            return Ok(());
        }
    }
    Err(AppError::InvalidInput(
        "Created order remains listed after cancellation checks".to_string(),
    ))
}

fn accepted_deal_id<'a>(
    confirmation: &'a OrderConfirmationResponse,
    reference: &str,
    epic: &str,
) -> Result<&'a str, AppError> {
    if confirmation.deal_reference != reference
        || confirmation.deal_status != Some(DealStatus::Accepted)
        || confirmation.epic.as_deref() != Some(epic)
    {
        return Err(AppError::InvalidInput(
            "Confirmation is not an accepted operation for the requested reference and EPIC"
                .to_string(),
        ));
    }
    confirmation
        .deal_id
        .as_deref()
        .filter(|id| !id.trim().is_empty())
        .ok_or_else(|| AppError::InvalidInput("Accepted confirmation has no deal ID".to_string()))
}

fn order_to_delete<'a>(
    confirmation: &'a OrderConfirmationResponse,
    reference: &str,
    epic: &str,
    orders: &WorkingOrdersResponse,
) -> Result<&'a str, AppError> {
    let deal_id = accepted_deal_id(confirmation, reference, epic)?;
    let mut matches = orders
        .working_orders
        .iter()
        .filter(|order| order.working_order_data.deal_id == deal_id);
    let order = matches.next().ok_or_else(|| {
        AppError::InvalidInput(
            "Created order is no longer listed as working; it may have executed or expired"
                .to_string(),
        )
    })?;
    if matches.next().is_some() || order.working_order_data.epic != epic {
        return Err(AppError::InvalidInput(
            "Working-order identity is inconsistent".to_string(),
        ));
    }
    Ok(deal_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // Synthetic fixtures, not observed account orders or production incidents.
    fn confirmation_fixture() -> Result<OrderConfirmationResponse, AppError> {
        Ok(serde_json::from_value(json!({
            "date": "2026-01-01T00:00:00", "status": "OPEN", "dealStatus": "ACCEPTED",
            "dealReference": "fixture-reference", "dealId": "fixture-created", "epic": "FIXTURE.EPIC"
        }))?)
    }

    fn orders_fixture(ids: &[&str]) -> Result<WorkingOrdersResponse, AppError> {
        let mut orders = Vec::new();
        for id in ids {
            orders.push(json!({
                "workingOrderData": {
                    "dealId": id, "epic": "FIXTURE.EPIC", "direction": "BUY",
                    "orderSize": 1.0, "orderLevel": 100.0, "timeInForce": "GOOD_TILL_CANCELLED",
                    "createdDate": "2026-01-01T00:00:00", "createdDateUTC": "2026-01-01T00:00:00",
                    "guaranteedStop": false, "orderType": "LIMIT", "currencyCode": "GBP", "dma": false
                },
                "marketData": {
                    "instrumentName": "Synthetic fixture", "exchangeId": "FIXTURE",
                    "expiry": "-", "marketStatus": "TRADEABLE", "epic": "FIXTURE.EPIC",
                    "instrumentType": "INDICES", "lotSize": 1.0,
                    "percentageChange": 0.0, "netChange": 0.0,
                    "updateTime": "00:00:00", "updateTimeUTC": "00:00:00",
                    "delayTime": 0, "streamingPricesAvailable": false, "scalingFactor": 1
                }
            }));
        }
        Ok(serde_json::from_value(json!({ "workingOrders": orders }))?)
    }

    #[test]
    fn test_order_to_delete_unrelated_first_order_selects_created_id() -> Result<(), AppError> {
        let confirmation = confirmation_fixture()?;
        let orders = orders_fixture(&["fixture-unrelated", "fixture-created"])?;
        assert_eq!(
            order_to_delete(&confirmation, "fixture-reference", "FIXTURE.EPIC", &orders)?,
            "fixture-created"
        );
        Ok(())
    }

    #[test]
    fn test_order_to_delete_missing_or_duplicate_created_order_errors() -> Result<(), AppError> {
        let confirmation = confirmation_fixture()?;
        for ids in [
            &[][..],
            &["fixture-unrelated"][..],
            &["fixture-created", "fixture-created"][..],
        ] {
            assert!(
                order_to_delete(
                    &confirmation,
                    "fixture-reference",
                    "FIXTURE.EPIC",
                    &orders_fixture(ids)?
                )
                .is_err()
            );
        }
        Ok(())
    }

    #[test]
    fn test_order_to_delete_rejected_or_mismatched_confirmation_errors() -> Result<(), AppError> {
        let orders = orders_fixture(&["fixture-created"])?;
        let original = confirmation_fixture()?;
        let mut rejected = original.clone();
        rejected.deal_status = Some(DealStatus::Rejected);
        let mut missing_status = original.clone();
        missing_status.deal_status = None;
        let mut wrong_reference = original.clone();
        wrong_reference.deal_reference = "fixture-other-reference".to_string();
        let mut wrong_epic = original.clone();
        wrong_epic.epic = Some("FIXTURE.OTHER".to_string());
        let mut missing_id = original.clone();
        missing_id.deal_id = None;
        let mut blank_id = original;
        blank_id.deal_id = Some(" ".to_string());
        for confirmation in [
            rejected,
            missing_status,
            wrong_reference,
            wrong_epic,
            missing_id,
            blank_id,
        ] {
            assert!(
                order_to_delete(&confirmation, "fixture-reference", "FIXTURE.EPIC", &orders)
                    .is_err()
            );
        }
        Ok(())
    }

    #[test]
    fn test_order_to_delete_mismatched_working_epic_errors() -> Result<(), AppError> {
        let confirmation = confirmation_fixture()?;
        let mut orders = orders_fixture(&["fixture-created"])?;
        for order in &mut orders.working_orders {
            order.working_order_data.epic = "FIXTURE.OTHER".to_string();
        }
        assert!(
            order_to_delete(&confirmation, "fixture-reference", "FIXTURE.EPIC", &orders).is_err()
        );
        Ok(())
    }
}
