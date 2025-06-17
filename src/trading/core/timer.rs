use std::time::Instant;

/// 交易时间测量器
pub struct TradeTimer {
    start_time: Instant,
    stage: String,
}

impl TradeTimer {
    /// 创建新的计时器
    pub fn new(stage: impl Into<String>) -> Self {
        Self {
            start_time: Instant::now(),
            stage: stage.into(),
        }
    }
    
    /// 记录当前阶段耗时并开始新阶段
    pub fn stage(&mut self, new_stage: impl Into<String>) {
        let elapsed = self.start_time.elapsed();
        println!(" {} 耗时: {:?}", self.stage, elapsed);
        
        self.start_time = Instant::now();
        self.stage = new_stage.into();
    }
    
    /// 完成计时并输出最终耗时
    pub fn finish(self) {
        let elapsed = self.start_time.elapsed();
        println!(" {} 耗时: {:?}", self.stage, elapsed);
    }
    
    /// 获取当前阶段的耗时（不重置计时器）
    pub fn elapsed(&self) -> std::time::Duration {
        self.start_time.elapsed()
    }
}

impl Drop for TradeTimer {
    fn drop(&mut self) {
        if !self.stage.is_empty() {
            let elapsed = self.start_time.elapsed();
            println!(" {} 耗时: {:?}", self.stage, elapsed);
        }
    }
} 