use criterion::{black_box, criterion_group, criterion_main, Criterion};
use runtime_repository::{RepositoryDiscovery, DiscoveryConfig};
use std::path::PathBuf;

fn bench_discovery(c: &mut Criterion) {
    let config = DiscoveryConfig {
        root_path: PathBuf::from("../../"), // the repo root
        max_depth: None,
        respect_gitignore: true,
    };
    let discovery = RepositoryDiscovery::new(config);

    c.bench_function("repository_discovery", |b| {
        b.iter(|| discovery.discover())
    });
}

criterion_group!(benches, bench_discovery);
criterion_main!(benches);
