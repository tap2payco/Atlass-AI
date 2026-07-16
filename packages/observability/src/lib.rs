pub trait MetricsRecorder: Send + Sync {
    fn record_counter(&self, name: &str, value: u64);
    fn record_gauge(&self, name: &str, value: f64);
    fn record_histogram(&self, name: &str, value: f64);
}

pub trait TraceRecorder: Send + Sync {
    fn start_span(&self, name: &str) -> String;
    fn end_span(&self, id: &str);
}

pub trait EventRecorder: Send + Sync {
    fn record_event(&self, name: &str, attributes: std::collections::HashMap<String, String>);
}

pub trait HealthReporter: Send + Sync {
    fn report_health(&self) -> bool;
}
