use crate::domain::MemoryRecord;

pub trait Scorer {
    fn score<T>(&self, record: &MemoryRecord<T>, current_score: f32) -> f32;
}

pub struct ImportanceScore;
impl Scorer for ImportanceScore {
    fn score<T>(&self, record: &MemoryRecord<T>, current_score: f32) -> f32 {
        current_score * record.metadata.importance
    }
}

pub struct ConfidenceScore;
impl Scorer for ConfidenceScore {
    fn score<T>(&self, record: &MemoryRecord<T>, current_score: f32) -> f32 {
        current_score * record.metadata.confidence
    }
}

pub struct QualityScore;
impl Scorer for QualityScore {
    fn score<T>(&self, record: &MemoryRecord<T>, current_score: f32) -> f32 {
        current_score * record.metadata.quality
    }
}

pub struct RankingEngine {
    scorers: Vec<Box<dyn Scorer + Send + Sync>>,
}

impl RankingEngine {
    pub fn new() -> Self { 
        Self {
            scorers: vec![
                Box::new(ImportanceScore),
                Box::new(ConfidenceScore),
                Box::new(QualityScore),
            ]
        } 
    }
    
    pub fn rank<T>(&self, mut candidates: Vec<MemoryRecord<T>>) -> Vec<MemoryRecord<T>> {
        candidates.sort_by(|a, b| {
            let mut score_a = 1.0;
            let mut score_b = 1.0;
            
            for scorer in &self.scorers {
                score_a = scorer.score(a, score_a);
                score_b = scorer.score(b, score_b);
            }
            
            score_b.partial_cmp(&score_a).unwrap_or(std::cmp::Ordering::Equal)
        });
        candidates
    }
}
