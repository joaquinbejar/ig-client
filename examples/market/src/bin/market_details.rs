//! Export details for an explicit comma-separated EPIC list from the catalogue.
//! Every requested EPIC must be returned exactly once before any file is written.

use ig_client::prelude::*;
use std::collections::{HashMap, HashSet};
use tracing::{info, warn};

/// Maximum EPICs in each detail request.
const BATCH_SIZE: usize = 25;

#[must_use = "invalid EPIC input must stop the request"]
fn validate_epics(epics: &[String]) -> Result<HashSet<&str>, AppError> {
    let mut expected = HashSet::with_capacity(epics.len());
    for epic in epics {
        if epic.trim().is_empty() || !expected.insert(epic.as_str()) {
            return Err(AppError::InvalidInput(
                "supply a nonempty comma-separated list of distinct EPICs".to_owned(),
            ));
        }
    }
    if expected.is_empty() {
        return Err(AppError::InvalidInput(
            "at least one EPIC is required".to_owned(),
        ));
    }
    Ok(expected)
}

/// Validate and order responses by their own EPIC, never by response position.
#[must_use = "incomplete or mismatched detail responses must not be exported"]
fn validate_details(
    epics: &[String],
    details: Vec<MarketDetails>,
) -> Result<Vec<MarketDetails>, AppError> {
    let expected = validate_epics(epics)?;
    let mut by_epic = HashMap::with_capacity(details.len());
    for detail in details {
        let epic = detail.instrument.epic.clone();
        if !expected.contains(epic.as_str()) {
            return Err(AppError::Deserialization(format!(
                "unexpected market details EPIC {epic}"
            )));
        }
        if by_epic.insert(epic.clone(), detail).is_some() {
            return Err(AppError::Deserialization(format!(
                "duplicate market details EPIC {epic}"
            )));
        }
    }
    epics
        .iter()
        .map(|epic| {
            by_epic.remove(epic).ok_or_else(|| {
                AppError::Deserialization(format!(
                    "missing market details for EPIC {epic}; export is incomplete"
                ))
            })
        })
        .collect()
}

#[tokio::main]
async fn main() -> IgResult<()> {
    setup_logger();
    let mut args = std::env::args().skip(1);
    let raw = args.next().ok_or_else(|| {
        AppError::InvalidInput(
            "usage: market_details EPIC[,EPIC...]; obtain EPICs with get_all_markets".to_owned(),
        )
    })?;
    if args.next().is_some() {
        return Err(AppError::InvalidInput(
            "pass one comma-separated EPIC argument".to_owned(),
        ));
    }
    let epics: Vec<String> = raw.split(',').map(|epic| epic.trim().to_owned()).collect();
    validate_epics(&epics)?;
    let client = Client::try_new()?;
    let mut all_details = Vec::with_capacity(epics.len());
    for chunk in epics.chunks(BATCH_SIZE) {
        let batch = match client.get_multiple_market_details(chunk).await {
            Ok(response) => response.market_details,
            Err(error) => {
                warn!(error = %error, "batch request failed; requesting each EPIC individually");
                let mut individual = Vec::with_capacity(chunk.len());
                for epic in chunk {
                    individual.push(client.get_market_details(epic).await?);
                }
                individual
            }
        };
        all_details.extend(validate_details(chunk, batch)?);
    }
    // Export full DTOs: the nested instrument EPIC is the authoritative identity,
    // with its expiry and lastDealingDate remaining distinct fields.
    let json = serde_json::to_string_pretty(&all_details)?;
    tokio::fs::create_dir_all("Data").await?;
    let filename = "Data/market_details.json";
    tokio::fs::write(filename, json).await?;
    info!(
        instruments = all_details.len(),
        filename, "complete requested market details saved"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Synthetic identity/expiry counterexamples, not recorded IG incidents.
    fn fixture_details(epic: &str, expiry: &str) -> Result<MarketDetails, serde_json::Error> {
        serde_json::from_value(serde_json::json!({
            "instrument": {
                "epic": epic,
                "name": "Synthetic fixture",
                "expiry": expiry,
                "contractSize": "1",
                "valueOfOnePip": "1",
                "expiryDetails": {"lastDealingDate": "2026-09-18T16:58:00"}
            },
            "snapshot": {"marketStatus": "TRADEABLE"},
            "dealingRules": {
                "marketOrderPreference": "AVAILABLE_DEFAULT_OFF",
                "trailingStopsPreference": "AVAILABLE"
            }
        }))
    }

    #[test]
    fn test_details_reordered_fixtures_preserve_epic_and_expiry() -> Result<(), AppError> {
        let epics = ["OP.D.FIXTURE.SEP.IP", "OP.D.FIXTURE.DEC.IP"].map(str::to_owned);
        let ordered = validate_details(
            &epics,
            vec![
                fixture_details("OP.D.FIXTURE.DEC.IP", "DEC-26")?,
                fixture_details("OP.D.FIXTURE.SEP.IP", "18-SEP-26")?,
            ],
        )?;
        let actual: Vec<_> = ordered
            .iter()
            .map(|detail| {
                (
                    detail.instrument.epic.as_str(),
                    detail.instrument.expiry.as_str(),
                )
            })
            .collect();
        assert_eq!(
            actual,
            vec![
                ("OP.D.FIXTURE.SEP.IP", "18-SEP-26"),
                ("OP.D.FIXTURE.DEC.IP", "DEC-26")
            ]
        );
        Ok(())
    }

    #[test]
    fn test_details_missing_duplicate_or_extra_fixtures_are_rejected() -> Result<(), AppError> {
        let epics = ["FIXTURE-A", "FIXTURE-B"].map(str::to_owned);
        for response in [
            vec![fixture_details("FIXTURE-A", "SEP-26")?],
            vec![
                fixture_details("FIXTURE-A", "SEP-26")?,
                fixture_details("FIXTURE-A", "DEC-26")?,
            ],
            vec![
                fixture_details("FIXTURE-A", "SEP-26")?,
                fixture_details("FIXTURE-B", "DEC-26")?,
                fixture_details("FIXTURE-C", "MAR-27")?,
            ],
        ] {
            assert!(matches!(
                validate_details(&epics, response),
                Err(AppError::Deserialization(_))
            ));
        }
        Ok(())
    }

    #[test]
    fn test_details_empty_or_duplicate_requested_epics_are_rejected() {
        for epics in [vec![], vec![" "], vec!["FIXTURE-A", "FIXTURE-A"]] {
            let epics: Vec<_> = epics.into_iter().map(str::to_owned).collect();
            assert!(matches!(
                validate_epics(&epics),
                Err(AppError::InvalidInput(_))
            ));
        }
    }
}
