use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DiagnosticSeverity {
    Info,
    Warning,
    Error,
    Fatal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CanonicalizationDiagnostic {
    DuplicateAlias(String),
    UnknownType(String),
    AmbiguousEntity(String),
    MissingCanonicalName(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RelationshipDiagnostic {
    InvalidRelationship(String),
    MissingEndpoint(String),
    OntologyViolation(String),
    DuplicateRelationship(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum KnowledgeDiagnostic {
    Canonicalization {
        severity: DiagnosticSeverity,
        diagnostic: CanonicalizationDiagnostic,
        context: String,
    },
    Relationship {
        severity: DiagnosticSeverity,
        diagnostic: RelationshipDiagnostic,
        context: String,
    },
    Structural {
        severity: DiagnosticSeverity,
        message: String,
    },
    Semantic {
        severity: DiagnosticSeverity,
        message: String,
    },
}
