//! An external implementor of the pre-extension trait must compile unchanged.

use async_trait::async_trait;
use ig_client::application::interfaces::order::OrderService;
use ig_client::error::AppError;
use ig_client::model::requests::{
    ClosePositionRequest, CreateOrderRequest, CreateWorkingOrderRequest, UpdatePositionRequest,
    UpdateWorkingOrderRequest,
};
use ig_client::model::responses::{
    ClosePositionResponse, CreateOrderResponse, CreateWorkingOrderResponse,
    OrderConfirmationResponse, SinglePositionResponse, UpdatePositionResponse,
};
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Default)]
struct LegacyOrderService {
    calls: AtomicUsize,
}

impl LegacyOrderService {
    fn unused<T>(&self) -> Result<T, AppError> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        Err(AppError::InvalidInput("unused fixture method".into()))
    }
}

#[async_trait]
impl OrderService for LegacyOrderService {
    async fn create_order(
        &self,
        _order: &CreateOrderRequest,
    ) -> Result<CreateOrderResponse, AppError> {
        self.unused()
    }

    async fn get_order_confirmation(
        &self,
        _deal_reference: &str,
    ) -> Result<OrderConfirmationResponse, AppError> {
        self.unused()
    }

    async fn get_order_confirmation_w_retry(
        &self,
        _deal_reference: &str,
        _retries: u64,
        _delay_ms: u64,
    ) -> Result<OrderConfirmationResponse, AppError> {
        self.unused()
    }

    async fn update_position(
        &self,
        _deal_id: &str,
        _update: &UpdatePositionRequest,
    ) -> Result<UpdatePositionResponse, AppError> {
        self.unused()
    }

    async fn update_level_in_position(
        &self,
        _deal_id: &str,
        _limit_level: Option<f64>,
    ) -> Result<UpdatePositionResponse, AppError> {
        self.unused()
    }

    async fn close_position(
        &self,
        _close_request: &ClosePositionRequest,
    ) -> Result<ClosePositionResponse, AppError> {
        self.unused()
    }

    async fn create_working_order(
        &self,
        _order: &CreateWorkingOrderRequest,
    ) -> Result<CreateWorkingOrderResponse, AppError> {
        self.unused()
    }

    async fn delete_working_order(&self, _deal_id: &str) -> Result<(), AppError> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    async fn get_position(&self, _deal_id: &str) -> Result<SinglePositionResponse, AppError> {
        self.unused()
    }

    async fn update_working_order(
        &self,
        _deal_id: &str,
        _update: &UpdateWorkingOrderRequest,
    ) -> Result<CreateWorkingOrderResponse, AppError> {
        self.unused()
    }
}

#[tokio::test]
async fn test_reference_cancellation_unsupported_default_never_calls_legacy_service() {
    let legacy = LegacyOrderService::default();
    let service: &dyn OrderService = &legacy;

    assert!(matches!(
        service.delete_working_order_with_reference("FAKE-DEAL").await,
        Err(AppError::InvalidInput(message))
            if message == "delete_working_order_with_reference is unsupported by this service implementation"
    ));
    assert_eq!(legacy.calls.load(Ordering::Relaxed), 0);

    assert!(service.delete_working_order("FAKE-DEAL").await.is_ok());
    assert_eq!(legacy.calls.load(Ordering::Relaxed), 1);
}
