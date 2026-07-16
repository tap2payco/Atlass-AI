use platform_contracts::execution::ValidationReport;

pub trait Validator<T> {
    type Diagnostic;

    fn validate(&self, item: &T) -> ValidationReport<Self::Diagnostic>;
}
