use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use yobu_daemon::{apps::Applications, search::SearchIndex};
use yobu_protocol::{Entry, Message};
use std::{collections::HashMap, hint::black_box, time::Duration};

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
                b.iter(|| index.search(black_box(query), 50, black_box(&[])))
            });
        }
    }
    group.finish();
}

fn show_message_serialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("show_message_json");
    group.sample_size(50);
    for count in [100, 1_000, 10_000] {
        let message = Message::Show {
            prompt: "Applications".into(),
            entries: synthetic_entries(count),
            positions: HashMap::new(),
            config: Default::default(),
            benchmark: false,
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
    show_message_serialization
);
criterion_main!(performance);
