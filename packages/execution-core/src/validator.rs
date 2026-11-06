use platform_contracts::execution::ValidationReport;

pub trait Validator<T>: Send + Sync {
    type Diagnostic: Send + Sync;

    fn validate(&self, item: &T) -> ValidationReport<Self::Diagnostic>;
}

pub struct CompositeValidator<D> {
    _marker: std::marker::PhantomData<D>,
}

impl<D> CompositeValidator<D> {
    pub fn aggregate(reports: Vec<ValidationReport<D>>) -> ValidationReport<D> {
        let mut errors = Vec::new();
        let mut warnings = Vec::new();
        let mut suggestions = Vec::new();

        for mut report in reports {
            errors.append(&mut report.errors);
            warnings.append(&mut report.warnings);
            suggestions.append(&mut report.suggestions);
        }

        ValidationReport {
            errors,
            warnings,
            suggestions,
        }
    }
}
