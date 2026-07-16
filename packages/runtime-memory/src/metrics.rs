pub struct MemoryMetrics {
    pub working_memory_size: u64,
    pub semantic_memory_size: u64,
    pub episodic_memory_size: u64,
    
    pub retrieval_latency_ms: u64,
    pub consolidation_latency_ms: u64,
    
    pub reinforcement_rate: f32,
    pub archive_rate: f32,
    pub compression_ratio: f32,
    pub retrieval_hit_rate: f32,
    
    pub average_quality: f32,
    pub average_stability: f32,
}

impl Default for MemoryMetrics {
    fn default() -> Self {
        Self {
            working_memory_size: 0,
            semantic_memory_size: 0,
            episodic_memory_size: 0,
            retrieval_latency_ms: 0,
            consolidation_latency_ms: 0,
            reinforcement_rate: 0.0,
            archive_rate: 0.0,
            compression_ratio: 1.0,
            retrieval_hit_rate: 0.0,
            average_quality: 0.0,
            average_stability: 0.0,
        }
    }
}
