use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use qmenu_daemon::apps::Applications;
use qmenu_protocol::{Entry, Message};
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

// This mirrors LauncherView.qml's current lowercase substring filter. It is a
// Rust baseline for comparing a future daemon-side matcher, not a QML runtime
// measurement. The display benchmark exercises the real QML path.
fn substring_filter<'a>(entries: &'a [Entry], query: &str, max_results: usize) -> Vec<&'a Entry> {
    let needle = query.trim().to_lowercase();
    if needle.is_empty() {
        return entries.iter().take(max_results).collect();
    }

    let mut matches: Vec<_> = entries
        .iter()
        .filter(|entry| {
            format!(
                "{} {}",
                entry.label,
                entry.description.as_deref().unwrap_or_default()
            )
            .to_lowercase()
            .contains(&needle)
        })
        .collect();
    matches.truncate(max_results);
    matches
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

fn search_baseline(c: &mut Criterion) {
    let mut group = c.benchmark_group("substring_search_rust_baseline");
    group.sample_size(50);
    for count in [100, 1_000, 10_000] {
        let entries = synthetic_entries(count);

        for (name, query) in [
            ("empty", "".to_owned()),
            ("common", "application".to_owned()),
            ("last_entry", format!("Application {}", count - 1)),
            ("no_match", "this does not exist".to_owned()),
        ] {
            group.bench_with_input(
                BenchmarkId::new(name, count),
                &(entries.as_slice(), query),
                |b, (entries, query)| {
                    b.iter(|| substring_filter(black_box(entries), black_box(query), 50))
                },
            );
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
    search_baseline,
    show_message_serialization
);
criterion_main!(performance);
