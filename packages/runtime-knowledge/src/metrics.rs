pub struct ExtractionMetrics {
    pub entities_found: u64,
}

pub struct CanonicalizationMetrics {
    pub aliases_resolved: u64,
}

pub struct RelationshipMetrics {
    pub edges_created: u64,
}

pub struct ConflictMetrics {
    pub conflicts_detected: u64,
}

pub struct InferenceMetrics {
    pub facts_generated: u64,
}

pub struct IndexMetrics {
    pub graph_updates: u64,
}

pub struct KnowledgeMetrics {
    pub extraction: ExtractionMetrics,
    pub canonicalization: CanonicalizationMetrics,
    pub relationship: RelationshipMetrics,
    pub conflict: ConflictMetrics,
    pub inference: InferenceMetrics,
    pub index: IndexMetrics,
}

impl Default for KnowledgeMetrics {
    fn default() -> Self {
        Self {
            extraction: ExtractionMetrics { entities_found: 0 },
            canonicalization: CanonicalizationMetrics { aliases_resolved: 0 },
            relationship: RelationshipMetrics { edges_created: 0 },
            conflict: ConflictMetrics { conflicts_detected: 0 },
            inference: InferenceMetrics { facts_generated: 0 },
            index: IndexMetrics { graph_updates: 0 },
        }
    }
}
