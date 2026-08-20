use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use std::{hint::black_box, time::Duration};
use yobu::config::SavedState;
use yobu::protocol::{Entry, Envelope};
use yobu::{apps::Applications, search::SearchIndex};

fn synthetic_entries(count: usize) -> Vec<Entry> {
    (0..count)
        .map(|index| Entry {
            id: format!("benchmark:{index}"),
            label: if index % 10 == 0 {
                format!("Terminal Emulator {index}")
            } else {
                format!("Application {index}")
            },
            description: Some(format!("Synthetic benchmark entry number {index}")),
            icon: Some("application-x-executable".into()),
            preview: None,
            value: format!("benchmark:{index}"),
        })
        .collect()
}

fn desktop_discovery(c: &mut Criterion) {
    let mut group = c.benchmark_group("desktop_entries");
    group.sample_size(20);
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(5));
    group.bench_function("discover", |b| {
        b.iter(|| black_box(Applications::discover()))
    });
    group.finish();
}

fn fuzzy_search(c: &mut Criterion) {
    let mut group = c.benchmark_group("fuzzy_search");
    group.sample_size(50);
    for count in [100, 1_000, 10_000] {
        let index = SearchIndex::new(synthetic_entries(count));

        for (name, query) in [
            ("empty", "".to_owned()),
            ("common", "application".to_owned()),
            ("last_entry", format!("Application {}", count - 1)),
            ("no_match", "this does not exist".to_owned()),
        ] {
            group.bench_with_input(BenchmarkId::new(name, count), &query, |b, query| {
                let state = SavedState::default();
                b.iter(|| index.search(black_box(query), 50, black_box(&state)))
            });
        }
    }
    group.finish();
}

fn menu_request_serialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("menu_request_json");
    group.sample_size(50);
    for count in [100, 1_000, 10_000] {
        let message = Envelope::Request {
            id: 1,
            method: "menus.create".into(),
            params: serde_json::json!({
                "prompt": "Applications",
                "entries": synthetic_entries(count),
            }),
        };
        group.throughput(Throughput::Elements(count as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(count),
            &message,
            |b, message| b.iter(|| serde_json::to_vec(black_box(message)).unwrap()),
        );
    }
    group.finish();
}

criterion_group!(
    performance,
    desktop_discovery,
    fuzzy_search,
    menu_request_serialization
);
criterion_main!(performance);
