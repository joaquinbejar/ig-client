use ig_client::utils::id::get_id;

#[test]
fn test_get_id_not_empty() {
    let id = get_id();
    assert!(!id.is_empty());
}

#[test]
fn test_get_id_length() {
    let id = get_id();
    assert_eq!(id.len(), 30);
}

#[test]
fn test_get_id_contains_valid_chars() {
    let id = get_id();

    // Deal references use nanoid's URL-safe alphabet: `[A-Za-z0-9_-]`.
    for c in id.chars() {
        assert!(
            c.is_ascii_alphanumeric() || c == '_' || c == '-',
            "Invalid character: {c}"
        );
    }
}

#[test]
fn test_get_id_uniqueness() {
    let id1 = get_id();
    let id2 = get_id();

    // IDs should be different (extremely high probability)
    assert_ne!(id1, id2);
}

#[test]
fn test_get_id_multiple_calls() {
    let mut ids = std::collections::HashSet::new();

    // Generate 100 IDs and ensure they're all unique
    for _ in 0..100 {
        let id = get_id();
        assert!(ids.insert(id), "Duplicate ID generated");
    }

    assert_eq!(ids.len(), 100);
}
