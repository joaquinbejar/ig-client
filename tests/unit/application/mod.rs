mod test_auth;
mod test_auth_flow;
mod test_client;
mod test_http_request;
// Exercises `Listener`, which decodes a `StreamingUpdate` and hands it to a
// user callback. The stream-pumping half (`Listener::spawn`) needs a
// `lightstreamer_rs::Updates`, which has no public constructor, so it is only
// reachable from the env-gated live tests.
mod test_key_pool;
#[cfg(feature = "streaming")]
mod test_listener;
