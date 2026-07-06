/******************************************************************************
   Author: Joaquín Béjar García
   Email: jb@taunais.com
   Date: 4/9/24
******************************************************************************/

//! Criterion benchmarks for the issue-46 performance work.
//!
//! Every case here is PURE and network-free so it runs deterministically:
//!
//! * `hierarchy_dedup`  — batched storage collection logic: HashMap dedup
//!   (used by `store_market_hierarchy`) vs a naive linear-scan dedup.
//! * `extract_markets`  — `extract_markets_from_hierarchy` borrowing (library,
//!   zero clones) vs the previous clone-per-level behaviour.
//! * `symbol_epic_map`  — `get_vec_db_entries` symbol→epic mapping: one-pass
//!   HashMap vs the previous O(symbols × entries) `find` scan.

use std::collections::{HashMap, HashSet};
use std::hint::black_box;

use criterion::{Criterion, criterion_group};

use ig_client::model::utils::extract_markets_from_hierarchy;
use ig_client::prelude::DBEntryResponse;
use ig_client::presentation::instrument::InstrumentType;
use ig_client::presentation::market::{MarketData, MarketNode};

// --------------------------------------------------------------------------
// Synthetic data builders (public DTOs only)
// --------------------------------------------------------------------------

fn make_market(i: usize) -> MarketData {
    MarketData {
        epic: format!("IX.D.SYMB{}.DAILY.IP", i),
        instrument_name: format!("Instrument {i}"),
        instrument_type: InstrumentType::Shares,
        expiry: "DFB".to_string(),
        high_limit_price: Some(100.0),
        low_limit_price: Some(50.0),
        market_status: "TRADEABLE".to_string(),
        net_change: Some(1.0),
        percentage_change: Some(0.5),
        update_time: Some("2024-01-01T00:00:00".to_string()),
        update_time_utc: Some("2024-01-01T00:00:00Z".to_string()),
        bid: Some(75.0),
        offer: Some(76.0),
    }
}

/// Builds a two-level hierarchy with `top * children_per * markets_per` markets
/// (plus `top * markets_per` markets at the top level).
fn make_hierarchy(top: usize, children_per: usize, markets_per: usize) -> Vec<MarketNode> {
    let mut counter = 0usize;
    (0..top)
        .map(|t| {
            let children = (0..children_per)
                .map(|c| {
                    let markets = (0..markets_per)
                        .map(|_| {
                            counter += 1;
                            make_market(counter)
                        })
                        .collect();
                    MarketNode {
                        id: format!("node-{t}-{c}"),
                        name: format!("Child {t}-{c}"),
                        children: vec![],
                        markets,
                    }
                })
                .collect();
            let markets = (0..markets_per)
                .map(|_| {
                    counter += 1;
                    make_market(counter)
                })
                .collect();
            MarketNode {
                id: format!("top-{t}"),
                name: format!("Top {t}"),
                children,
                markets,
            }
        })
        .collect()
}

fn make_entries(n: usize, symbols: usize) -> Vec<DBEntryResponse> {
    (0..n)
        .map(|i| DBEntryResponse {
            symbol: format!("SYM{}", i % symbols),
            epic: format!("IX.D.SYM{}.{}.IP", i % symbols, i),
            expiry: "DFB".to_string(),
            ..DBEntryResponse::default()
        })
        .collect()
}

// --------------------------------------------------------------------------
// (a) Hierarchy storage collection: HashMap dedup vs linear-scan dedup
// --------------------------------------------------------------------------

/// Mirrors `dedupe_by_key`: first-position, last-data, O(n) via a HashMap.
fn dedup_hashmap(keys: &[String]) -> Vec<String> {
    let mut index: HashMap<String, usize> = HashMap::with_capacity(keys.len());
    let mut deduped: Vec<String> = Vec::with_capacity(keys.len());
    for key in keys {
        if let Some(&i) = index.get(key) {
            deduped[i] = key.clone();
        } else {
            index.insert(key.clone(), deduped.len());
            deduped.push(key.clone());
        }
    }
    deduped
}

/// Naive O(n^2) dedup by linear scan, representing the cost of not indexing.
fn dedup_linear(keys: &[String]) -> Vec<String> {
    let mut deduped: Vec<String> = Vec::new();
    for key in keys {
        if let Some(pos) = deduped.iter().position(|k| k == key) {
            deduped[pos] = key.clone();
        } else {
            deduped.push(key.clone());
        }
    }
    deduped
}

fn bench_hierarchy_dedup(c: &mut Criterion) {
    // 8_000 rows, half of them duplicate ids (as a full-exchange refresh can
    // contain the same node/epic under multiple parents).
    let keys: Vec<String> = (0..8_000).map(|i| format!("KEY{}", i % 4_000)).collect();

    let mut group = c.benchmark_group("hierarchy_dedup");
    group.bench_function("hashmap_O(n)", |b| {
        b.iter(|| dedup_hashmap(black_box(&keys)))
    });
    group.bench_function("linear_O(n^2)", |b| {
        b.iter(|| dedup_linear(black_box(&keys)))
    });
    group.finish();
}

// --------------------------------------------------------------------------
// (b) extract_markets_from_hierarchy: borrow (new) vs clone (old)
// --------------------------------------------------------------------------

/// The previous clone-heavy implementation, kept here as the A/B baseline.
fn extract_markets_cloning(nodes: &[MarketNode]) -> Vec<MarketData> {
    let mut all_markets = Vec::new();
    for node in nodes {
        all_markets.extend(node.markets.clone());
        if !node.children.is_empty() {
            all_markets.extend(extract_markets_cloning(&node.children));
        }
    }
    all_markets
}

fn bench_extract_markets(c: &mut Criterion) {
    // ~10_100 markets across a two-level tree.
    let hierarchy = make_hierarchy(10, 100, 10);

    let mut group = c.benchmark_group("extract_markets");
    group.bench_function("borrow_new", |b| {
        b.iter(|| {
            let markets = extract_markets_from_hierarchy(black_box(&hierarchy));
            black_box(markets.len())
        })
    });
    group.bench_function("clone_old", |b| {
        b.iter(|| {
            let markets = extract_markets_cloning(black_box(&hierarchy));
            black_box(markets.len())
        })
    });
    group.finish();
}

// --------------------------------------------------------------------------
// (c) symbol -> epic mapping: one-pass HashMap (new) vs O(n^2) find (old)
// --------------------------------------------------------------------------

/// One-pass build, mirroring the new `get_vec_db_entries` mapping.
fn map_symbols_onepass(entries: &[DBEntryResponse]) -> HashMap<String, (String, String)> {
    let mut map: HashMap<String, (String, String)> = HashMap::new();
    for entry in entries {
        if entry.symbol.is_empty() || entry.epic.is_empty() {
            continue;
        }
        map.entry(entry.symbol.clone())
            .or_insert_with(|| (entry.epic.clone(), entry.expiry.clone()));
    }
    map
}

/// The previous approach: collect unique symbols, then `find` per symbol —
/// O(symbols × entries).
fn map_symbols_linear(entries: &[DBEntryResponse]) -> HashMap<String, String> {
    let unique: HashSet<String> = entries
        .iter()
        .map(|e| e.symbol.clone())
        .filter(|s| !s.is_empty())
        .collect();
    let mut map: HashMap<String, String> = HashMap::new();
    for symbol in unique {
        if let Some(entry) = entries
            .iter()
            .find(|e| e.symbol == symbol && !e.epic.is_empty())
        {
            map.insert(symbol, entry.epic.clone());
        }
    }
    map
}

fn bench_symbol_epic_map(c: &mut Criterion) {
    // 6_000 entries across 1_500 distinct symbols.
    let entries = make_entries(6_000, 1_500);

    let mut group = c.benchmark_group("symbol_epic_map");
    group.bench_function("onepass_new", |b| {
        b.iter(|| map_symbols_onepass(black_box(&entries)))
    });
    group.bench_function("find_old", |b| {
        b.iter(|| map_symbols_linear(black_box(&entries)))
    });
    group.finish();
}

criterion_group!(
    benches,
    bench_hierarchy_dedup,
    bench_extract_markets,
    bench_symbol_epic_map,
);
