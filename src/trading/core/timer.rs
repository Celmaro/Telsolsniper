use std::time::Instant;

/// Trade time measurement tool
#[derive(Clone)]
pub struct TradeTimer {
    start_time: Instant,
    stage: String,
}

impl TradeTimer {
    /// Create a new timer
    pub fn new(stage: impl Into<String>) -> Self {
        Self { start_time: Instant::now(), stage: stage.into() }
    }

    /// Record current stage time and start a new stage
    pub fn stage(&mut self, new_stage: impl Into<String>) {
        let elapsed = self.start_time.elapsed();
        println!(" {} time cost: {:?}", self.stage, elapsed);

        self.start_time = Instant::now();
        self.stage = new_stage.into();
    }

    /// Complete timing and output final time cost
    pub fn finish(mut self) {
        let elapsed = self.start_time.elapsed();
        println!(" {} time cost: {:?}", self.stage, elapsed);
        self.stage.clear(); // Clear stage to avoid duplicate printing in Drop
    }

    /// Get the elapsed time of current stage (without resetting the timer)
    pub fn elapsed(&self) -> std::time::Duration {
        self.start_time.elapsed()
    }
}

impl Drop for TradeTimer {
    fn drop(&mut self) {
        if !self.stage.is_empty() {
            let elapsed = self.start_time.elapsed();
            println!(" {} time cost: {:?}", self.stage, elapsed);
        }
    }
}
