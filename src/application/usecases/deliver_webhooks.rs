// Use case: deliver_webhooks.

use crate::application::context::AppContext;
use crate::domain::entities::event::Event;
use crate::domain::value_objects::ids::EventId;
use crate::domain::value_objects::timestamps::Timestamp;
use crate::infrastructure::db::dto::WebhookDeliveryRow;
use serde::Serialize;

/// Delivers queued webhook callbacks and updates delivery status.
pub struct DeliverWebhooksUseCase;

#[derive(Debug)]
pub enum DeliverWebhooksError {
    Storage(String),
    Delivery(String),
}

#[derive(Debug, Clone)]
pub struct DeliverWebhooksResult {
    pub processed: usize,
    pub delivered: usize,
    pub failed: usize,
}

#[derive(Debug, Serialize)]
struct WebhookPayload {
    pub job_id: String,
    pub event_id: String,
    pub event_name: String,
    pub prev_state: String,
    pub next_state: String,
    pub occurred_at: String,
}

impl DeliverWebhooksUseCase {
    /// Deliver due webhook callbacks once and return processing stats.
    pub async fn run_once(
        ctx: &AppContext,
        now: Timestamp,
        limit: u32,
    ) -> Result<DeliverWebhooksResult, DeliverWebhooksError> {
        // Step 1: Load due webhook deliveries.
        let rows = ctx
            .repos
            .webhook_delivery
            .list_due(now.as_inner(), limit)
            .await
            .map_err(|e| DeliverWebhooksError::Storage(format!("{e:?}")))?;
        let total = rows.len();

        let mut delivered = 0;
        let mut failed = 0;

        // Step 2: Build an HTTP client with the configured timeout.
        let timeout =
            std::time::Duration::from_millis(ctx.settings.webhook_delivery.request_timeout_ms);
        let client = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .map_err(|e| DeliverWebhooksError::Delivery(e.to_string()))?;

        // Step 3: Deliver each webhook and update its delivery status.
        for mut delivery in rows {
            let result = Self::deliver_one(ctx, &client, now, &mut delivery).await;
            match result {
                Ok(status) => {
                    if status == DeliveryStatus::Delivered {
                        delivered += 1;
                    } else if status == DeliveryStatus::Failed {
                        failed += 1;
                    }
                }
                Err(err) => return Err(err),
            }
        }

        // Step 4: Return summary stats for observability.
        Ok(DeliverWebhooksResult {
            processed: total,
            delivered,
            failed,
        })
    }

    /// Run the webhook delivery loop continuously at a fixed interval.
    pub async fn run_loop(
        ctx: &AppContext,
        poll_interval: time::Duration,
        limit: u32,
        mut shutdown: tokio::sync::watch::Receiver<bool>,
    ) -> Result<(), DeliverWebhooksError> {
        // Step 1: Loop until shutdown is triggered.
        loop {
            if *shutdown.borrow() {
                break;
            }

            // Step 2: Run a delivery pass.
            let _ = Self::run_once(ctx, Timestamp::now_utc(), limit).await?;

            // Step 3: Sleep until the next pass or shutdown.
            let sleep_duration =
                std::time::Duration::from_millis(poll_interval.whole_milliseconds().max(0) as u64);
            tokio::select! {
                _ = shutdown.changed() => {
                    if *shutdown.borrow() {
                        break;
                    }
                }
                _ = tokio::time::sleep(sleep_duration) => {}
            }
        }

        // Step 4: Exit cleanly.
        Ok(())
    }

    async fn deliver_one(
        ctx: &AppContext,
        client: &reqwest::Client,
        now: Timestamp,
        delivery: &mut WebhookDeliveryRow,
    ) -> Result<DeliveryStatus, DeliverWebhooksError> {
        // Step 1: Load the event payload.
        let Some(event) = ctx
            .repos
            .event
            .get(EventId(delivery.event_id))
            .await
            .map_err(|e| DeliverWebhooksError::Storage(format!("{e:?}")))?
        else {
            Self::mark_failed(delivery, now, "event_missing".to_string());
            ctx.repos
                .webhook_delivery
                .update(delivery)
                .await
                .map_err(|e| DeliverWebhooksError::Storage(format!("{e:?}")))?;
            return Ok(DeliveryStatus::Failed);
        };

        // Step 2: Send the webhook request.
        let payload = Self::build_payload(&event);
        let response = client
            .post(&delivery.target_url)
            .json(&payload)
            .send()
            .await;

        // Step 3: Update delivery status based on the HTTP response.
        match response {
            Ok(resp) if resp.status().is_success() => {
                Self::mark_delivered(delivery, now, resp.status().as_u16() as i32);
                ctx.repos
                    .webhook_delivery
                    .update(delivery)
                    .await
                    .map_err(|e| DeliverWebhooksError::Storage(format!("{e:?}")))?;
                Ok(DeliveryStatus::Delivered)
            }
            Ok(resp) => {
                let status = resp.status().as_u16() as i32;
                let status =
                    Self::mark_retry(delivery, now, Some(status), "http_error".to_string(), ctx);
                ctx.repos
                    .webhook_delivery
                    .update(delivery)
                    .await
                    .map_err(|e| DeliverWebhooksError::Storage(format!("{e:?}")))?;
                Ok(status)
            }
            Err(err) => {
                let status = Self::mark_retry(delivery, now, None, err.to_string(), ctx);
                ctx.repos
                    .webhook_delivery
                    .update(delivery)
                    .await
                    .map_err(|e| DeliverWebhooksError::Storage(format!("{e:?}")))?;
                Ok(status)
            }
        }
    }

    fn build_payload(event: &Event) -> WebhookPayload {
        WebhookPayload {
            job_id: event.job_id.0.to_string(),
            event_id: event.id.0.to_string(),
            event_name: event.event_name.as_str().to_string(),
            prev_state: event.prev_state.as_str().to_string(),
            next_state: event.next_state.as_str().to_string(),
            occurred_at: event
                .timestamp
                .as_inner()
                .format(&time::format_description::well_known::Rfc3339)
                .unwrap_or_default(),
        }
    }

    fn mark_delivered(delivery: &mut WebhookDeliveryRow, now: Timestamp, status: i32) {
        delivery.status = "delivered".to_string();
        delivery.response_status = Some(status);
        delivery.last_error = None;
        delivery.delivered_at = Some(now.as_inner());
        delivery.updated_at = now.as_inner();
        delivery.next_attempt_at = None;
    }

    fn mark_failed(delivery: &mut WebhookDeliveryRow, now: Timestamp, error: String) {
        delivery.status = "failed".to_string();
        delivery.last_error = Some(error);
        delivery.updated_at = now.as_inner();
        delivery.next_attempt_at = None;
    }

    fn mark_retry(
        delivery: &mut WebhookDeliveryRow,
        now: Timestamp,
        response_status: Option<i32>,
        error: String,
        ctx: &AppContext,
    ) -> DeliveryStatus {
        let attempt = delivery.attempt.saturating_add(1) as u32;
        delivery.attempt = attempt as i32;
        delivery.response_status = response_status;
        delivery.last_error = Some(error);
        delivery.updated_at = now.as_inner();
        delivery.delivered_at = None;

        let max_attempts = ctx.settings.webhook_delivery.max_attempts;
        if attempt >= max_attempts {
            delivery.status = "failed".to_string();
            delivery.next_attempt_at = None;
            return DeliveryStatus::Failed;
        }

        let backoff_ms = compute_backoff_ms(
            attempt,
            ctx.settings.webhook_delivery.backoff_initial_ms,
            ctx.settings.webhook_delivery.backoff_max_ms,
        );
        delivery.status = "pending".to_string();
        delivery.next_attempt_at =
            Some(now.as_inner() + time::Duration::milliseconds(backoff_ms as i64));
        DeliveryStatus::Pending
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DeliveryStatus {
    Pending,
    Delivered,
    Failed,
}

fn compute_backoff_ms(attempt: u32, base_ms: u64, max_ms: u64) -> u64 {
    let exp = 2u64.saturating_pow(attempt.saturating_sub(1));
    let raw = base_ms.saturating_mul(exp);
    raw.min(max_ms)
}

#[cfg(test)]
mod tests {
    use super::compute_backoff_ms;

    #[test]
    fn given_attempts_when_compute_backoff_should_grow_and_cap() {
        assert_eq!(compute_backoff_ms(1, 500, 5000), 500);
        assert_eq!(compute_backoff_ms(2, 500, 5000), 1000);
        assert_eq!(compute_backoff_ms(3, 500, 5000), 2000);
        assert_eq!(compute_backoff_ms(10, 500, 5000), 5000);
    }
}
