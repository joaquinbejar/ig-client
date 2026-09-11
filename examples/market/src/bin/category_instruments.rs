//! Fetch exactly one page of a category returned by the `categories` example.
//! Usage: category_instruments CATEGORY_ID [PAGE_NUMBER=0] [PAGE_SIZE=150]
//! Use `get_all_markets` for complete enumeration across every account category.

use ig_client::prelude::*;
use ig_client::utils::setup_logger;
use tracing::info;

#[must_use = "invalid page arguments must stop the request"]
fn parse_args(mut args: impl Iterator<Item = String>) -> Result<(String, u32, u32), AppError> {
    let category = args
        .next()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            AppError::InvalidInput(
                "usage: category_instruments CATEGORY_ID [PAGE_NUMBER=0] [PAGE_SIZE=150]"
                    .to_owned(),
            )
        })?;
    let page = args
        .next()
        .map(|value| {
            value.parse::<u32>().map_err(|_| {
                AppError::InvalidInput("page number must be a nonnegative integer".to_owned())
            })
        })
        .transpose()?
        .unwrap_or(0);
    let size = args
        .next()
        .map(|value| {
            value.parse::<u32>().map_err(|_| {
                AppError::InvalidInput("page size must be an integer between 1 and 1000".to_owned())
            })
        })
        .transpose()?
        .unwrap_or(150);
    if !(1..=1000).contains(&size) || args.next().is_some() {
        return Err(AppError::InvalidInput(
            "expected CATEGORY_ID, an optional page number and a page size between 1 and 1000"
                .to_owned(),
        ));
    }
    Ok((category, page, size))
}

#[tokio::main]
async fn main() -> IgResult<()> {
    setup_logger();
    let (category_id, page_number, page_size) = parse_args(std::env::args().skip(1))?;
    let client = Client::try_new()?;
    let categories = client.get_categories().await?;
    if !categories
        .categories
        .iter()
        .any(|category| category.code == category_id)
    {
        return Err(AppError::InvalidInput(format!(
            "category {category_id} is not enabled for this account; use a code from the categories example"
        )));
    }
    let result = client
        .get_category_instruments(&category_id, Some(page_number), Some(page_size))
        .await?;
    info!(category = %category_id, page_number, instruments = result.len(), "one category page received; this is not a complete catalogue");
    for instrument in result.iter() {
        info!(epic = %instrument.epic, expiry = %instrument.expiry, expiry_timestamp = ?instrument.expiry_timestamp, "listed instrument");
    }
    // Retain the whole response, including metadata, and keep pages separate.
    let json = serde_json::to_string_pretty(&result)?;
    let safe_category: String = category_id
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '_' || character == '-' {
                character
            } else {
                '_'
            }
        })
        .collect();
    let filename =
        format!("Data/category_{safe_category}_page_{page_number}_size_{page_size}.json");
    tokio::fs::create_dir_all("Data").await?;
    tokio::fs::write(&filename, json).await?;
    info!(filename, "category page and metadata saved");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_page_args_synthetic_valid_values_and_defaults() -> Result<(), AppError> {
        assert_eq!(
            parse_args(["FIXTURE".to_owned()].into_iter())?,
            ("FIXTURE".to_owned(), 0, 150)
        );
        assert_eq!(
            parse_args(["FIXTURE", "2", "1000"].map(str::to_owned).into_iter())?,
            ("FIXTURE".to_owned(), 2, 1000)
        );
        Ok(())
    }

    #[test]
    fn test_page_args_synthetic_invalid_values_are_rejected() {
        for args in [
            vec![],
            vec![" "],
            vec!["FIXTURE", "bad"],
            vec!["FIXTURE", "-1"],
            vec!["FIXTURE", "0", "0"],
            vec!["FIXTURE", "0", "1001"],
            vec!["FIXTURE", "0", "bad"],
            vec!["FIXTURE", "0", "1", "extra"],
        ] {
            assert!(matches!(
                parse_args(args.into_iter().map(str::to_owned)),
                Err(AppError::InvalidInput(_))
            ));
        }
    }
}
