use ig_client::application::market_hierarchy::extract_markets_from_hierarchy;
use ig_client::prelude::{MarketData, MarketNode};
use ig_client::presentation::instrument::InstrumentType;

fn create_test_market(epic: &str, name: &str) -> MarketData {
    MarketData {
        bid: None,
        epic: epic.to_string(),
        expiry: "-".to_string(),
        high_limit_price: None,
        instrument_name: name.to_string(),
        instrument_type: InstrumentType::Shares,
        low_limit_price: None,
        market_status: "TRADEABLE".to_string(),
        net_change: Some(0.0),
        offer: None,
        percentage_change: Some(0.0),
        update_time: Some("00:00:00".to_string()),
        update_time_utc: Some("2024-01-01T00:00:00".to_string()),
        ..Default::default()
    }
}

#[test]
fn test_extract_markets_from_empty_hierarchy() {
    let nodes: Vec<MarketNode> = vec![];
    let markets = extract_markets_from_hierarchy(&nodes);
    assert!(markets.is_empty());
}

#[test]
fn test_extract_markets_from_single_node_with_market() {
    let market = create_test_market("TEST.EPIC", "Test Market");
    let node = MarketNode {
        id: "node1".to_string(),
        name: "Node 1".to_string(),
        children: vec![],
        markets: vec![market.clone()],
    };

    let nodes = [node];
    let markets = extract_markets_from_hierarchy(&nodes);
    assert_eq!(markets.len(), 1);
    assert_eq!(markets[0].epic, "TEST.EPIC");
}

#[test]
fn test_extract_markets_from_multiple_nodes() {
    let market1 = create_test_market("EPIC1", "Market 1");
    let market2 = create_test_market("EPIC2", "Market 2");

    let node1 = MarketNode {
        id: "node1".to_string(),
        name: "Node 1".to_string(),
        children: vec![],
        markets: vec![market1.clone()],
    };

    let node2 = MarketNode {
        id: "node2".to_string(),
        name: "Node 2".to_string(),
        children: vec![],
        markets: vec![market2.clone()],
    };

    let nodes = [node1, node2];
    let markets = extract_markets_from_hierarchy(&nodes);
    assert_eq!(markets.len(), 2);
    assert_eq!(markets[0].epic, "EPIC1");
    assert_eq!(markets[1].epic, "EPIC2");
}

#[test]
fn test_extract_markets_from_nested_hierarchy() {
    let market1 = create_test_market("EPIC1", "Market 1");
    let market2 = create_test_market("EPIC2", "Market 2");
    let market3 = create_test_market("EPIC3", "Market 3");

    let child_node = MarketNode {
        id: "child1".to_string(),
        name: "Child Node".to_string(),
        children: vec![],
        markets: vec![market2.clone()],
    };

    let parent_node = MarketNode {
        id: "parent1".to_string(),
        name: "Parent Node".to_string(),
        children: vec![child_node],
        markets: vec![market1.clone()],
    };

    let sibling_node = MarketNode {
        id: "sibling1".to_string(),
        name: "Sibling Node".to_string(),
        children: vec![],
        markets: vec![market3.clone()],
    };

    let nodes = [parent_node, sibling_node];
    let markets = extract_markets_from_hierarchy(&nodes);
    assert_eq!(markets.len(), 3);
    assert_eq!(markets[0].epic, "EPIC1");
    assert_eq!(markets[1].epic, "EPIC2");
    assert_eq!(markets[2].epic, "EPIC3");
}

#[test]
fn test_extract_markets_from_node_without_markets() {
    let node = MarketNode {
        id: "node1".to_string(),
        name: "Node 1".to_string(),
        children: vec![],
        markets: vec![],
    };

    let nodes = [node];
    let markets = extract_markets_from_hierarchy(&nodes);
    assert!(markets.is_empty());
}

#[test]
fn test_extract_markets_from_deeply_nested_hierarchy() {
    let market1 = create_test_market("EPIC1", "Market 1");
    let market2 = create_test_market("EPIC2", "Market 2");
    let market3 = create_test_market("EPIC3", "Market 3");

    let level3 = MarketNode {
        id: "level3".to_string(),
        name: "Level 3".to_string(),
        children: vec![],
        markets: vec![market3.clone()],
    };

    let level2 = MarketNode {
        id: "level2".to_string(),
        name: "Level 2".to_string(),
        children: vec![level3],
        markets: vec![market2.clone()],
    };

    let level1 = MarketNode {
        id: "level1".to_string(),
        name: "Level 1".to_string(),
        children: vec![level2],
        markets: vec![market1.clone()],
    };

    let nodes = [level1];
    let markets = extract_markets_from_hierarchy(&nodes);
    assert_eq!(markets.len(), 3);
    assert_eq!(markets[0].epic, "EPIC1");
    assert_eq!(markets[1].epic, "EPIC2");
    assert_eq!(markets[2].epic, "EPIC3");
}

#[test]
fn test_extract_markets_preserves_market_data() {
    let market = MarketData {
        bid: Some(100.5),
        epic: "TEST.EPIC".to_string(),
        expiry: "DEC-24".to_string(),
        high_limit_price: Some(105.0),
        instrument_name: "Test Instrument".to_string(),
        instrument_type: InstrumentType::Commodities,
        low_limit_price: Some(95.0),
        market_status: "TRADEABLE".to_string(),
        net_change: Some(2.5),
        offer: Some(101.0),
        percentage_change: Some(2.5),
        update_time: Some("12:34:56".to_string()),
        update_time_utc: Some("2024-01-01T12:34:56".to_string()),
        ..Default::default()
    };

    let node = MarketNode {
        id: "node1".to_string(),
        name: "Node 1".to_string(),
        children: vec![],
        markets: vec![market.clone()],
    };

    let nodes = [node];
    let markets = extract_markets_from_hierarchy(&nodes);
    assert_eq!(markets.len(), 1);

    let extracted = &markets[0];
    assert_eq!(extracted.bid, Some(100.5));
    assert_eq!(extracted.epic, "TEST.EPIC");
    assert_eq!(extracted.expiry, "DEC-24");
    assert_eq!(extracted.high_limit_price, Some(105.0));
    assert_eq!(extracted.instrument_name, "Test Instrument");
    assert_eq!(extracted.instrument_type, InstrumentType::Commodities);
    assert_eq!(extracted.low_limit_price, Some(95.0));
    assert_eq!(extracted.market_status, "TRADEABLE");
    assert_eq!(extracted.net_change, Some(2.5));
    assert_eq!(extracted.offer, Some(101.0));
    assert_eq!(extracted.percentage_change, Some(2.5));
    assert_eq!(extracted.update_time, Some("12:34:56".to_string()));
    assert_eq!(
        extracted.update_time_utc,
        Some("2024-01-01T12:34:56".to_string())
    );
}
