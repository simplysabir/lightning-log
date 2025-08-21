use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use lightning_log::*;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

// Standard library logging for comparison
use log::{info, warn};
use std::sync::Once;

static INIT: Once = Once::new();

fn ensure_logger_init() {
    INIT.call_once(|| {
        let _ = init_lightning_log_with_capacity(100000);
        env_logger::init();
    });
}

// Benchmark lightning log vs standard println!
fn bench_lightning_vs_println(c: &mut Criterion) {
    ensure_logger_init();

    let mut group = c.benchmark_group("logging_comparison");
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(10000);

    // Lightning log benchmark
    group.bench_function("lightning_log", |b| {
        b.iter(|| {
            let volume = black_box(100.02f64);
            let price = black_box(20000.0f64);
            let flag = black_box(true);
            let symbol = black_box("AAPL");

            lightning_info!(1001, volume, price, flag, symbol);
        })
    });

    // Standard println! benchmark (simulating typical logging)
    group.bench_function("println_macro", |b| {
        b.iter(|| {
            let volume = black_box(100.02f64);
            let price = black_box(20000.0f64);
            let flag = black_box(true);
            let symbol = black_box("AAPL");
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();

            println!("[INFO] ts:{} volume:{} price:{} flag:{} symbol:{}",
                    timestamp, volume, price, flag, symbol);
        })
    });

        // Standard log crate benchmark (note: this just does macro expansion)
    // In real usage, you'd need to add a subscriber for actual output
    group.bench_function("log_crate", |b| {
        b.iter(|| {
            let volume = black_box(100.02f64);
            let price = black_box(20000.0f64);
            let flag = black_box(true);
            let symbol = black_box("AAPL");

            info!("volume:{} price:{} flag:{} symbol:{}", volume, price, flag, symbol);
        })
    });

    group.finish();
}

// Benchmark different data types
fn bench_data_types(c: &mut Criterion) {
    ensure_logger_init();

    let mut group = c.benchmark_group("data_types");
    group.measurement_time(Duration::from_secs(5));

    group.bench_function("integers", |b| {
        b.iter(|| {
            lightning_info!(2001, black_box(42i32), black_box(123i64), black_box(255u8));
        })
    });

    group.bench_function("floats", |b| {
        b.iter(|| {
            lightning_info!(2002, black_box(3.14159f32), black_box(2.718281828f64));
        })
    });

    group.bench_function("strings", |b| {
        b.iter(|| {
            lightning_info!(2003, black_box("AAPL"), black_box("GOOGL"));
        })
    });

    group.bench_function("mixed_types", |b| {
        b.iter(|| {
            lightning_info!(2004,
                black_box(100.5f64),
                black_box(42i32),
                black_box(true),
                black_box("MSFT")
            );
        })
    });

    group.finish();
}

// Benchmark message complexity
fn bench_message_complexity(c: &mut Criterion) {
    ensure_logger_init();

    let mut group = c.benchmark_group("message_complexity");
    group.measurement_time(Duration::from_secs(5));

    for arg_count in [1, 2, 4, 8, 16].iter() {
        group.bench_with_input(BenchmarkId::new("arguments", arg_count), arg_count, |b, &arg_count| {
            match arg_count {
                1 => b.iter(|| lightning_info!(3001, black_box(1.0f64))),
                2 => b.iter(|| lightning_info!(3002, black_box(1.0f64), black_box(2.0f64))),
                4 => b.iter(|| lightning_info!(3004,
                    black_box(1.0f64), black_box(2.0f64),
                    black_box(3.0f64), black_box(4.0f64))),
                8 => b.iter(|| lightning_info!(3008,
                    black_box(1.0f64), black_box(2.0f64), black_box(3.0f64), black_box(4.0f64),
                    black_box(5.0f64), black_box(6.0f64), black_box(7.0f64), black_box(8.0f64))),
                16 => b.iter(|| lightning_info!(3016,
                    black_box(1.0f64), black_box(2.0f64), black_box(3.0f64), black_box(4.0f64),
                    black_box(5.0f64), black_box(6.0f64), black_box(7.0f64), black_box(8.0f64),
                    black_box(9.0f64), black_box(10.0f64), black_box(11.0f64), black_box(12.0f64),
                    black_box(13.0f64), black_box(14.0f64), black_box(15.0f64), black_box(16.0f64))),
                _ => unreachable!(),
            }
        });
    }

    group.finish();
}

// Benchmark under high throughput
fn bench_high_throughput(c: &mut Criterion) {
    ensure_logger_init();

    let mut group = c.benchmark_group("high_throughput");
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(100);

    group.bench_function("burst_1k_messages", |b| {
        b.iter(|| {
            for i in 0..1000 {
                lightning_info!(4001, black_box(i as f64), black_box(i * 2), black_box(i % 2 == 0));
            }
        })
    });

    group.bench_function("burst_10k_messages", |b| {
        b.iter(|| {
            for i in 0..10000 {
                lightning_info!(4002, black_box(i as f64));
            }
        })
    });

    group.finish();
}

// Latency distribution benchmark
fn bench_latency_distribution(c: &mut Criterion) {
    ensure_logger_init();

    let mut group = c.benchmark_group("latency_distribution");
    group.measurement_time(Duration::from_secs(15));
    group.sample_size(50000);

    group.bench_function("single_message_latency", |b| {
        b.iter(|| {
            lightning_info!(5001, black_box(42.0f64), black_box(true), black_box("TEST"));
        })
    });

    group.finish();
}

// Memory allocation benchmark
fn bench_memory_pattern(c: &mut Criterion) {
    ensure_logger_init();

    let mut group = c.benchmark_group("memory_patterns");
    group.measurement_time(Duration::from_secs(5));

    // Test with pre-sized strings vs dynamic strings
    let static_str = "STATIC_SYMBOL";
    let dynamic_string = format!("DYNAMIC_{}", 42);

    group.bench_function("static_strings", |b| {
        b.iter(|| {
            lightning_info!(6001, black_box(100.5f64), black_box(static_str));
        })
    });

    group.bench_function("dynamic_strings", |b| {
        b.iter(|| {
            lightning_info!(6002, black_box(100.5f64), black_box(dynamic_string.as_str()));
        })
    });

    group.finish();
}

// Concurrent logging benchmark
fn bench_concurrent_logging(c: &mut Criterion) {
    ensure_logger_init();

    let mut group = c.benchmark_group("concurrent_logging");
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(100);

    group.bench_function("single_thread", |b| {
        b.iter(|| {
            for i in 0..1000 {
                lightning_info!(7001, black_box(i as f64));
            }
        })
    });

    group.bench_function("multi_thread_4", |b| {
        b.iter(|| {
            let handles: Vec<_> = (0..4).map(|thread_id| {
                std::thread::spawn(move || {
                    for i in 0..250 {
                        lightning_info!(7002, black_box(thread_id), black_box(i as f64));
                    }
                })
            }).collect();

            for handle in handles {
                handle.join().unwrap();
            }
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_lightning_vs_println,
    bench_data_types,
    bench_message_complexity,
    bench_high_throughput,
    bench_latency_distribution,
    bench_memory_pattern,
    bench_concurrent_logging
);

criterion_main!(benches);
