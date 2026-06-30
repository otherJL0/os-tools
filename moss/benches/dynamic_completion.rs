use std::ffi::OsString;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use moss::package;

fn criterion_benchmark(c: &mut Criterion) {
    let prefix = OsString::from("lib");

    c.bench_with_input(
        BenchmarkId::new(
            "prefix_completer(package::Flags::default().with_available()",
            prefix.to_string_lossy(),
        ),
        &prefix,
        |b, prefix| {
            b.iter(|| {
                let client_name = package::Flags::default().with_available();
                move |prefix: &std::ffi::OsStr| {
                    let Some(prefix) = prefix.to_str() else {
                        return vec![];
                    };
                    if prefix.is_empty() {
                        return vec![];
                    }
                    generate_results(client(client_name), flags, prefix)
                }
            }(prefix));
        },
    );
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
