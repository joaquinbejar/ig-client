/******************************************************************************
   Author: Joaquín Béjar García
   Email: jb@taunais.com
   Date: 20/10/25
******************************************************************************/

//! Market navigation hierarchy traversal.
//!
//! These helpers drive HTTP calls through `Client` (via the
//! [`MarketService`](crate::application::interfaces::market::MarketService)
//! trait), so they live in the application layer rather than the pure `model`
//! layer.

use crate::application::client::Client;
use crate::application::interfaces::market::MarketService;
use crate::error::AppError;
use crate::model::responses::MarketNavigationResponse;
use crate::presentation::market::{MarketData, MarketNode};
use std::future::Future;
use std::pin::Pin;
use tracing::{debug, error, info};

/// Builds a market hierarchy recursively by traversing the navigation tree
///
/// # Arguments
/// * `client` - Reference to the IG client
/// * `node_id` - Optional node ID to start from (None for root)
/// * `depth` - Current depth in the hierarchy (used to prevent infinite recursion)
///
/// # Returns
/// Vector of MarketNode representing the hierarchy at this level
///
/// # Errors
/// Returns [`AppError`] when a navigation request fails with a non-recoverable
/// error (rate limits and generic API errors degrade to partial results).
pub fn build_market_hierarchy<'a>(
    client: &'a Client,
    node_id: Option<&'a str>,
    depth: usize,
) -> Pin<Box<dyn Future<Output = Result<Vec<MarketNode>, AppError>> + 'a>> {
    Box::pin(async move {
        // Limit the depth to avoid infinite loops
        if depth > 7 {
            debug!("Reached maximum depth of 5, stopping recursion");
            return Ok(Vec::new());
        }

        // Get the nodes and markets at the current level
        let navigation: MarketNavigationResponse = match node_id {
            Some(id) => {
                debug!("Getting navigation node: {}", id);
                match client.get_market_navigation_node(id).await {
                    Ok(response) => {
                        debug!(
                            "Response received for node {}: {} nodes, {} markets",
                            id,
                            response.nodes.len(),
                            response.markets.len()
                        );
                        response
                    }
                    Err(e) => {
                        error!("Error getting node {}: {:?}", id, e);
                        // If we hit a rate limit, return empty results instead of failing
                        if matches!(e, AppError::RateLimitExceeded | AppError::Unexpected(_)) {
                            info!("Rate limit or API error encountered, returning partial results");
                            return Ok(Vec::new());
                        }
                        return Err(e);
                    }
                }
            }
            None => {
                debug!("Getting top-level navigation nodes");
                match client.get_market_navigation().await {
                    Ok(response) => {
                        debug!(
                            "Response received for top-level nodes: {} nodes, {} markets",
                            response.nodes.len(),
                            response.markets.len()
                        );
                        response
                    }
                    Err(e) => {
                        error!("Error getting top-level nodes: {:?}", e);
                        return Err(e);
                    }
                }
            }
        };

        let mut nodes = Vec::new();

        // Process all nodes at this level
        let nodes_to_process = navigation.nodes;

        // Process nodes sequentially with rate limiting
        // This is important to respect the API rate limits
        // By processing nodes sequentially, we allow the rate limiter
        // to properly control the flow of requests
        for node in nodes_to_process.into_iter() {
            // Recursively get the children of this node
            match build_market_hierarchy(client, Some(&node.id), depth + 1).await {
                Ok(children) => {
                    info!("Adding node {} with {} children", node.name, children.len());
                    nodes.push(MarketNode {
                        id: node.id.clone(),
                        name: node.name.clone(),
                        children,
                        markets: Vec::new(),
                    });
                }
                Err(e) => {
                    error!("Error building hierarchy for node {}: {:?}", node.id, e);
                    // Continue with other nodes even if one fails
                    if depth < 7 {
                        nodes.push(MarketNode {
                            id: node.id.clone(),
                            name: format!("{} (error: {})", node.name, e),
                            children: Vec::new(),
                            markets: Vec::new(),
                        });
                    }
                }
            }
        }

        // Process all markets in this node
        let markets_to_process = navigation.markets;
        for market in markets_to_process {
            debug!("Adding market: {}", market.instrument_name);
            nodes.push(MarketNode {
                id: market.epic.clone(),
                name: market.instrument_name.clone(),
                children: Vec::new(),
                markets: vec![market],
            });
        }

        Ok(nodes)
    })
}

/// Recursively extract all markets from the hierarchy into a flat list of
/// borrowed references.
///
/// The hierarchy owns the [`MarketData`]; callers only need read access, so
/// this returns `&MarketData` and performs no clones (previously every market
/// was deep-cloned at each recursion level). Markets are collected depth-first:
/// a node's own markets precede its descendants', preserving the previous
/// ordering. The returned references borrow `nodes`.
#[must_use]
pub fn extract_markets_from_hierarchy(nodes: &[MarketNode]) -> Vec<&MarketData> {
    // Pre-size once with the exact market count to avoid intermediate per-level
    // Vec allocations and reallocations.
    let mut all_markets = Vec::with_capacity(count_markets(nodes));
    collect_markets(nodes, &mut all_markets);
    all_markets
}

/// Counts every market in the hierarchy (including descendants) so the flat
/// output vector can be pre-allocated exactly once.
fn count_markets(nodes: &[MarketNode]) -> usize {
    nodes
        .iter()
        .map(|node| node.markets.len() + count_markets(&node.children))
        .sum()
}

/// Appends borrowed market references into `out` depth-first, reusing a single
/// accumulator instead of allocating a Vec per recursion level.
fn collect_markets<'a>(nodes: &'a [MarketNode], out: &mut Vec<&'a MarketData>) {
    for node in nodes {
        out.extend(node.markets.iter());
        if !node.children.is_empty() {
            collect_markets(&node.children, out);
        }
    }
}
