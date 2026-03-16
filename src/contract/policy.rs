#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionPolicy {
    pub max_parallel_microvms_per_run: u32,
    pub max_stage_retries: u32,
    pub stage_timeout_secs: u32,
    pub cancel_grace_period_secs: u32,
}

impl ExecutionPolicy {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.max_parallel_microvms_per_run == 0 {
            return Err("max_parallel_microvms_per_run must be > 0");
        }
        if self.stage_timeout_secs == 0 {
            return Err("stage_timeout_secs must be > 0");
        }
        if self.cancel_grace_period_secs == 0 {
            return Err("cancel_grace_period_secs must be > 0");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::ExecutionPolicy;

    #[test]
    fn validates_happy_path() {
        let policy = ExecutionPolicy {
            max_parallel_microvms_per_run: 8,
            max_stage_retries: 1,
            stage_timeout_secs: 900,
            cancel_grace_period_secs: 20,
        };
        assert!(policy.validate().is_ok());
    }

    #[test]
    fn rejects_invalid_limits() {
        let policy = ExecutionPolicy {
            max_parallel_microvms_per_run: 0,
            max_stage_retries: 1,
            stage_timeout_secs: 900,
            cancel_grace_period_secs: 20,
        };
        assert!(policy.validate().is_err());
    }
}
