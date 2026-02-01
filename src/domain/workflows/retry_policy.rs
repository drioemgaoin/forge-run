use time::Duration;

/// Retry policy for jobs that can be re-queued after failures.
#[derive(Debug, Clone)]
pub struct RetryPolicy {
    pub max_retries: u8,
    pub base_delay_ms: u64,
    pub max_delay_ms: u64,
    pub jitter_ms: u64,
}

impl RetryPolicy {
    /// Return the next backoff delay for a given retry attempt.
    ///
    /// `attempt` is the retry number starting at 1 for the first retry.
    pub fn next_delay(&self, attempt: u8, jitter_seed: u64) -> Duration {
        // Step 1: Compute the exponential delay (2^(attempt-1)).
        let attempt = attempt.max(1) as u32;
        let raw = self
            .base_delay_ms
            .saturating_mul(2_u64.saturating_pow(attempt - 1));

        // Step 2: Cap at the max delay to avoid unbounded backoff.
        let capped = raw.min(self.max_delay_ms);

        // Step 3: Optionally add a bounded jitter to spread retries.
        if self.jitter_ms == 0 {
            return Duration::milliseconds(capped as i64);
        }

        let jitter = (jitter_seed % self.jitter_ms).min(self.jitter_ms);
        Duration::milliseconds((capped + jitter) as i64)
    }

    /// Returns `true` when another retry is allowed.
    pub fn can_retry(&self, current_retry_count: u8) -> bool {
        // Step 1: Compare the current retry count against the limit.
        current_retry_count < self.max_retries
    }
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 3,
            base_delay_ms: 1_000,
            max_delay_ms: 30_000,
            jitter_ms: 250,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::RetryPolicy;

    #[test]
    fn given_first_retry_when_next_delay_called_should_use_base_delay() {
        let policy = RetryPolicy::default();
        let delay = policy.next_delay(1, 0);
        assert_eq!(delay.whole_milliseconds(), 1_000);
    }

    #[test]
    fn given_large_retry_when_next_delay_called_should_cap_at_max() {
        let policy = RetryPolicy::default();
        let delay = policy.next_delay(10, 0);
        assert_eq!(delay.whole_milliseconds(), 30_000);
    }
}
