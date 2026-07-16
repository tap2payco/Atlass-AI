use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum RetrievalIntent {
    ExplainCode,
    BugInvestigation,
    Planning,
    Refactoring,
    Documentation,
    TestGeneration,
    ArchitectureReview,
    DependencyAnalysis,
    SecurityAudit,
    Custom(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum RetrievalStrategy {
    Fast,
    Balanced,
    Comprehensive,
    Exhaustive,
    Offline,
    LowToken,
    HighAccuracy,
}
