# Working-order cancellation references

`OrderService::delete_working_order_with_reference(deal_id)` preserves IG's
cancellation acknowledgement instead of discarding it. The method returns the
existing `CreateWorkingOrderResponse`, whose `deal_reference` identifies the
submission for a later `get_order_confirmation` read. Both types are already
available through `ig_client::prelude`.

This method is available from 0.18.1. Consumers pinned to 0.18.0, including
ig-engine, must upgrade to 0.18.1 or later.

## Submission and reconciliation

The SDK sends `DELETE /workingorders/otc/{dealId}`, API version 2, with no request
body. It uses the existing trading rate class and `RequestPolicy::SingleAttempt`:
at most one business request, without status retries, key rotation, redirects,
protocol retries, or post-send authentication replay. Initial login or proactive
session refresh may still happen before dispatch.

Retain the original working-order deal ID and cancellation intent before
dispatch, then persist the acknowledgement reference if one is returned. A
successful acknowledgement establishes submission, not confirmed deletion; use
the confirmation endpoint to determine the outcome. The working-order deal ID
and acknowledgement reference serve different purposes and are not substitutes.

An HTTP, transport, or response-decoding error can leave the outcome uncertain.
If the acknowledgement is lost, its reference may be unavailable. Reconcile the
working order and available broker evidence; do not send another cancellation
just to recover a reference. The helper adds neither idempotency nor a request
deadline. A separate invocation is another request.

## Compatibility

- `delete_working_order(deal_id) -> Result<(), AppError>` keeps its signature. The
  SDK delegates to the new method exactly once and discards only the successful
  acknowledgement; errors propagate unchanged. Its `Ok(())` also acknowledges
  submission rather than confirmed deletion.
- Existing external `OrderService` implementations need no new method to
  compile, including implementations used through `dyn OrderService`. The new
  trait default returns `AppError::InvalidInput` without calling the legacy
  method or causing a request. Implementors must override it to provide the
  actual acknowledgement; they must not fabricate a reference.
- The response DTO, existing error variants, dependencies, and feature flags are
  unchanged. No new prelude export is needed.

Local regression coverage extends the existing mutation matrix to both
cancellation methods and checks exact request counts, returned references,
headers, empty request bodies, and typed failures. A separate external-implementor
fixture verifies that the default has no effect and the legacy method still
works. These tests use synthetic data and do not establish live broker outcomes.

See [the mutation request policy](./MUTATION_REQUEST_POLICY.md) for the underlying
single-attempt contract and its limits. That document records the earlier 0.18.0
implementation and validation history.
