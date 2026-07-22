use ig_client::application::interfaces::listener::Listener;
use ig_client::application::streaming_convert::StreamingUpdate;
use std::collections::HashMap;
use std::fmt::{self, Display};
use std::sync::{Arc, Mutex};

// Test data structure that implements required traits
#[derive(Debug, Clone)]
struct TestData {
    value: String,
}

impl Display for TestData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "TestData({})", self.value)
    }
}

impl From<&StreamingUpdate> for TestData {
    fn from(update: &StreamingUpdate) -> Self {
        TestData {
            value: update.item_name.clone().unwrap_or_default(),
        }
    }
}

/// Builds an update carrying only an item name, which is all `TestData` reads.
fn update(item_name: &str, item_pos: usize) -> StreamingUpdate {
    StreamingUpdate {
        item_name: Some(item_name.to_string()),
        item_pos,
        is_snapshot: false,
        fields: HashMap::new(),
        changed_fields: HashMap::new(),
    }
}

#[test]
fn test_listener_new() {
    let _listener = Listener::<TestData>::new(|data| {
        assert!(!data.value.is_empty());
        Ok(())
    });

    // Just verify it was created without panicking
}

#[test]
fn test_listener_on_item_update() {
    let called = Arc::new(Mutex::new(false));
    let called_clone = Arc::clone(&called);

    let listener = Listener::<TestData>::new(move |data| {
        *called_clone
            .lock()
            .expect("the callback mutex is never poisoned in this test") = true;
        assert!(!data.value.is_empty());
        Ok(())
    });

    listener.on_item_update(&update("TEST_ITEM", 1));

    assert!(
        *called
            .lock()
            .expect("the callback mutex is never poisoned in this test")
    );
}

#[test]
fn test_listener_multiple_updates() {
    let counter = Arc::new(Mutex::new(0));
    let counter_clone = Arc::clone(&counter);

    let listener = Listener::<TestData>::new(move |_data| {
        *counter_clone
            .lock()
            .expect("the callback mutex is never poisoned in this test") += 1;
        Ok(())
    });

    listener.on_item_update(&update("TEST1", 1));
    listener.on_item_update(&update("TEST2", 2));
    listener.on_item_update(&update("TEST3", 3));

    assert_eq!(
        *counter
            .lock()
            .expect("the callback mutex is never poisoned in this test"),
        3
    );
}

#[test]
fn test_listener_thread_safety() {
    use std::thread;

    let counter = Arc::new(Mutex::new(0));
    let counter_clone = Arc::clone(&counter);

    let listener = Arc::new(Listener::<TestData>::new(move |_data| {
        *counter_clone
            .lock()
            .expect("the callback mutex is never poisoned in this test") += 1;
        Ok(())
    }));

    let mut handles = vec![];

    for i in 0..5 {
        let listener_clone = Arc::clone(&listener);
        let handle = thread::spawn(move || {
            listener_clone.on_item_update(&update(&format!("THREAD_{i}"), i));
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().expect("worker thread should not panic");
    }

    assert_eq!(
        *counter
            .lock()
            .expect("the callback mutex is never poisoned in this test"),
        5
    );
}

#[test]
fn test_listener_with_different_data() {
    let values = Arc::new(Mutex::new(Vec::new()));
    let values_clone = Arc::clone(&values);

    let listener = Listener::<TestData>::new(move |data| {
        values_clone
            .lock()
            .expect("the callback mutex is never poisoned in this test")
            .push(data.value.clone());
        Ok(())
    });

    listener.on_item_update(&update("first", 1));
    listener.on_item_update(&update("second", 2));
    listener.on_item_update(&update("third", 3));

    let collected = values
        .lock()
        .expect("the callback mutex is never poisoned in this test");
    assert_eq!(collected.len(), 3);
    assert_eq!(collected[0], "first");
    assert_eq!(collected[1], "second");
    assert_eq!(collected[2], "third");
}

#[test]
fn test_listener_error_handling() {
    let after = Arc::new(Mutex::new(0));
    let counter = Arc::clone(&after);

    let listener = Listener::<TestData>::new(move |_data| {
        *counter
            .lock()
            .expect("the callback mutex is never poisoned in this test") += 1;
        Err(ig_client::error::AppError::InvalidInput(
            "Test error".to_string(),
        ))
    });

    // A failing callback must be contained: no panic, and the next update is
    // still delivered.
    listener.on_item_update(&update("ERROR_TEST", 1));
    listener.on_item_update(&update("ERROR_TEST_2", 2));

    assert_eq!(
        *after
            .lock()
            .expect("the callback mutex is never poisoned in this test"),
        2
    );
}
