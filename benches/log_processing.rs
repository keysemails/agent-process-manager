use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use agent_process_manager::logs::patterns::detect_patterns;
use agent_process_manager::logs::{LogEntry, LogStorage};
use agent_process_manager::test_utils::test_utils::create_test_db;
use tokio::runtime::Runtime;

fn pattern_detection_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("pattern_detection");
    
    // Test different log line complexities
    let test_lines = vec![
        ("simple", "This is a simple log line"),
        ("with_port", "Server started on port 8080"),
        ("with_url", "Connecting to https://api.example.com/v1/users"),
        ("with_error", "ERROR: Failed to connect to database"),
        ("with_file", "Loading config from /etc/app/config.json"),
        ("complex", "ERROR: Failed to connect to https://db.example.com:5432 - check /var/log/app.log"),
        ("very_long", &"x".repeat(1000)),
    ];
    
    for (name, line) in test_lines {
        group.bench_with_input(
            BenchmarkId::new("detect_patterns", name),
            line,
            |b, line| b.iter(|| detect_patterns(black_box(line)))
        );
    }
    
    group.finish();
}

fn log_storage_benchmark(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("log_storage");
    
    // Setup
    let (storage, _temp_dir) = rt.block_on(async {
        let (pool, temp_dir) = create_test_db().await.unwrap();
        let storage = LogStorage::new(pool).await.unwrap();
        (storage, temp_dir)
    });
    
    // Benchmark single log insertion
    group.bench_function("store_single_log", |b| {
        b.to_async(&rt).iter(|| async {
            let entry = LogEntry {
                id: None,
                process_id: "bench-process".to_string(),
                timestamp: chrono::Utc::now().timestamp_millis() as f64 / 1000.0,
                raw_line: "Benchmark log line with some content".to_string(),
                clean_line: "Benchmark log line with some content".to_string(),
                level: Some("info".to_string()),
                patterns: Some(serde_json::json!([])),
            };
            storage.store_log(entry).await.unwrap();
        });
    });
    
    // Benchmark batch insertion
    group.bench_function("store_100_logs", |b| {
        b.to_async(&rt).iter(|| async {
            for i in 0..100 {
                let entry = LogEntry {
                    id: None,
                    process_id: "bench-process".to_string(),
                    timestamp: chrono::Utc::now().timestamp_millis() as f64 / 1000.0 + i as f64,
                    raw_line: format!("Benchmark log line {}", i),
                    clean_line: format!("Benchmark log line {}", i),
                    level: Some("info".to_string()),
                    patterns: Some(serde_json::json!([])),
                };
                storage.store_log(entry).await.unwrap();
            }
        });
    });
    
    // Setup data for query benchmarks
    rt.block_on(async {
        for i in 0..1000 {
            let entry = LogEntry {
                id: None,
                process_id: "query-bench-process".to_string(),
                timestamp: chrono::Utc::now().timestamp_millis() as f64 / 1000.0 + i as f64,
                raw_line: format!("Log line {} with port {}", i, 8000 + (i % 10)),
                clean_line: format!("Log line {} with port {}", i, 8000 + (i % 10)),
                level: Some(if i % 3 == 0 { "error" } else { "info" }.to_string()),
                patterns: Some(serde_json::json!([{"type": "port", "value": 8000 + (i % 10)}])),
            };
            storage.store_log(entry).await.unwrap();
        }
    });
    
    // Benchmark queries
    group.bench_function("query_all_logs", |b| {
        b.to_async(&rt).iter(|| async {
            let query = agent_process_manager::logs::LogQuery {
                process_id: "query-bench-process".to_string(),
                level: None,
                start_time: None,
                end_time: None,
                pattern: None,
                limit: None,
                offset: None,
            };
            storage.query_logs(query).await.unwrap();
        });
    });
    
    group.bench_function("query_by_level", |b| {
        b.to_async(&rt).iter(|| async {
            let query = agent_process_manager::logs::LogQuery {
                process_id: "query-bench-process".to_string(),
                level: Some(agent_process_manager::logs::LogLevel::Error),
                start_time: None,
                end_time: None,
                pattern: None,
                limit: None,
                offset: None,
            };
            storage.query_logs(query).await.unwrap();
        });
    });
    
    group.bench_function("query_with_limit", |b| {
        b.to_async(&rt).iter(|| async {
            let query = agent_process_manager::logs::LogQuery {
                process_id: "query-bench-process".to_string(),
                level: None,
                start_time: None,
                end_time: None,
                pattern: None,
                limit: Some(50),
                offset: Some(100),
            };
            storage.query_logs(query).await.unwrap();
        });
    });
    
    group.finish();
}

fn large_log_processing(c: &mut Criterion) {
    let mut group = c.benchmark_group("large_logs");
    group.sample_size(10); // Reduce sample size for expensive operations
    
    // Generate large log lines
    let sizes = vec![100, 1000, 10000];
    
    for size in sizes {
        let line = format!(
            "Large log with {} chars containing port 8080, URL https://example.com, error Failed, path /tmp/file.txt",
            "x".repeat(size)
        );
        
        group.bench_with_input(
            BenchmarkId::new("process_large_line", size),
            &line,
            |b, line| b.iter(|| detect_patterns(black_box(line)))
        );
    }
    
    group.finish();
}

criterion_group!(
    benches,
    pattern_detection_benchmark,
    log_storage_benchmark,
    large_log_processing
);
criterion_main!(benches);