use shared::context::{ContextPackage, ValidationDiagnostic, ValidationDiagnosticLevel};

pub struct ValidationLayer;

impl ValidationLayer {
    pub fn validate(package: &ContextPackage) -> Vec<ValidationDiagnostic> {
        let mut diagnostics = Vec::new();

        // 1. Budget check
        if package.budget.tokens_used > package.budget.total_tokens_allocated {
            diagnostics.push(ValidationDiagnostic {
                level: ValidationDiagnosticLevel::Error,
                code: "BUDGET_EXCEEDED".to_string(),
                message: format!("Tokens used {} exceeds allocation {}", package.budget.tokens_used, package.budget.total_tokens_allocated),
                fragment_id: None,
            });
        }

        // 2. Fatal Errors for provenance and confidence
        for fragment in &package.fragments {
            if fragment.provenance.is_empty() {
                diagnostics.push(ValidationDiagnostic {
                    level: ValidationDiagnosticLevel::Error, // HARD FAILURE
                    code: "MISSING_PROVENANCE".to_string(),
                    message: format!("Fragment {} is missing provenance tracking", fragment.id),
                    fragment_id: Some(fragment.id.clone()),
                });
            }

            if fragment.confidence < 0.0 || fragment.confidence > 1.0 {
                diagnostics.push(ValidationDiagnostic {
                    level: ValidationDiagnosticLevel::Error, // HARD FAILURE
                    code: "INVALID_CONFIDENCE".to_string(),
                    message: format!("Fragment {} has invalid confidence: {}", fragment.id, fragment.confidence),
                    fragment_id: Some(fragment.id.clone()),
                });
            }
        }

        // 3. Stale Context warning (e.g. freshness > 1 hour old)
        for fragment in &package.fragments {
            if let Ok(elapsed) = fragment.freshness.elapsed() {
                if elapsed.as_secs() > 3600 {
                    diagnostics.push(ValidationDiagnostic {
                        level: ValidationDiagnosticLevel::Warning,
                        code: "STALE_CONTEXT".to_string(),
                        message: format!("Fragment {} is over 1 hour old", fragment.id),
                        fragment_id: Some(fragment.id.clone()),
                    });
                }
            }
        }

        diagnostics
    }
}
