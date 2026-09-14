use crate::error::AppError;
use crate::model::requests::{
    ClosePositionRequest, CreateOrderRequest, CreateWorkingOrderRequest, UpdatePositionRequest,
    UpdateWorkingOrderRequest,
};
use crate::model::responses::{
    ClosePositionResponse, CreateOrderResponse, CreateWorkingOrderResponse,
    OrderConfirmationResponse, SinglePositionResponse, UpdatePositionResponse,
};

use async_trait::async_trait;

#[async_trait]
/// Service for creating, updating, and managing trading orders with the IG Markets API
///
/// This trait defines the interface for interacting with the IG Markets order endpoints,
/// allowing clients to create new orders, get order confirmations, update existing positions,
/// and close positions.
///
/// The SDK [`Client`](crate::application::client::Client) sends each creation,
/// amendment, cancellation or close at most once. It does not retry or replay
/// that mutation on HTTP, transport or authentication errors. Initial session
/// establishment may happen before the send. An error after sending can leave
/// the broker outcome unknown; reconcile it before submitting another mutation.
/// Confirmation and position reads retain their finite retry behavior.
pub trait OrderService: Send + Sync {
    /// Creates a new order
    async fn create_order(
        &self,
        order: &CreateOrderRequest,
    ) -> Result<CreateOrderResponse, AppError>;

    /// Gets the confirmation of an order
    async fn get_order_confirmation(
        &self,
        deal_reference: &str,
    ) -> Result<OrderConfirmationResponse, AppError>;

    /// Gets the confirmation of an order with retry logic
    async fn get_order_confirmation_w_retry(
        &self,
        deal_reference: &str,
        retries: u64,
        delay_ms: u64,
    ) -> Result<OrderConfirmationResponse, AppError>;

    /// Updates an existing position
    async fn update_position(
        &self,
        deal_id: &str,
        update: &UpdatePositionRequest,
    ) -> Result<UpdatePositionResponse, AppError>;

    ///  Asynchronously updates the limit level of a position in a specified deal.
    ///  
    ///  # Parameters
    ///  - `deal_id`: A reference to a string slice representing the unique identifier of the deal
    ///    whose position is to be updated.
    ///  - `limit_level`: An optional `f64` value specifying the new limit level for the position.
    ///    If `None`, the limit level will not be updated.
    ///  
    ///  # Returns
    ///  - `Result<UpdatePositionResponse, AppError>`:
    ///    - On success, returns an `UpdatePositionResponse` containing details of the updated position.
    ///    - On failure, returns an `AppError` indicating the error encountered during the operation.
    ///  
    ///  # Errors
    ///  This function returns an `AppError` in case of:
    ///  - Invalid `deal_id` (e.g., deal doesn't exist).
    ///  - Backend service issues or database failures.
    ///  - Input validation errors for the `limit_level`.
    ///  
    ///  
    ///  # Notes
    ///  Ensure that the passed `deal_id` exists, and the `limit_level` (if provided) adheres
    ///  to any required constraints specific to the application's domain logic.
    ///  
    ///
    async fn update_level_in_position(
        &self,
        deal_id: &str,
        limit_level: Option<f64>,
    ) -> Result<UpdatePositionResponse, AppError>;

    /// Closes an existing position
    async fn close_position(
        &self,
        close_request: &ClosePositionRequest,
    ) -> Result<ClosePositionResponse, AppError>;

    /// Creates a new working order
    async fn create_working_order(
        &self,
        order: &CreateWorkingOrderRequest,
    ) -> Result<CreateWorkingOrderResponse, AppError>;

    /// Deletes a working order based on the provided deal ID.
    ///
    /// # Parameters
    /// - `deal_id`: A `String` representing the deal ID of the working order that needs to be deleted.
    ///
    /// # Returns
    /// - `Result<(), AppError>`:
    ///   - On success, the function returns `Ok(())` indicating that the working order was successfully deleted.
    ///   - On failure, it returns `Err(AppError)` containing the error details that occurred during the deletion process.
    ///
    /// # Errors
    /// This function will return an `AppError` in the following scenarios:
    /// - If the deletion operation fails due to invalid deal ID.
    /// - If there are connectivity issues with the database or external services.
    /// - If the calling user does not have permission to delete the specified working order.
    ///
    async fn delete_working_order(&self, deal_id: &str) -> Result<(), AppError>;

    /// Gets a single position by deal ID
    ///
    /// # Arguments
    /// * `deal_id` - The deal ID of the position to retrieve
    ///
    /// # Returns
    /// * `Ok(SinglePositionResponse)` - The position details and market data
    /// * `Err(AppError)` - If the request fails
    async fn get_position(&self, deal_id: &str) -> Result<SinglePositionResponse, AppError>;

    /// Updates an existing working order
    ///
    /// # Arguments
    /// * `deal_id` - The deal ID of the working order to update
    /// * `update` - The update parameters
    ///
    /// # Returns
    /// * `Ok(CreateWorkingOrderResponse)` - The updated working order confirmation
    /// * `Err(AppError)` - If the request fails
    async fn update_working_order(
        &self,
        deal_id: &str,
        update: &UpdateWorkingOrderRequest,
    ) -> Result<CreateWorkingOrderResponse, AppError>;
}
