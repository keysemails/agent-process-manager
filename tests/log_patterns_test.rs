use agent_process_manager::logs::{PatternDetector, DetectedPattern};
use proptest::prelude::*;
use test_case::test_case;

#[test]
fn test_port_detection() {
    let detector = PatternDetector::new();
    
    let test_cases = vec![
        ("Server listening on port 8080", vec![8080]),
        ("Starting on ports 3000 and 4000", vec![3000]),
        ("Listening on 0.0.0.0:9090", vec![9090]),
        ("Port: 443 (HTTPS)", vec![443]),
        ("No ports here", vec![]),
        ("Port 99999 is invalid", vec![]), // Out of valid range
        ("server started on 8080", vec![8080]),
    ];
    
    for (input, expected_ports) in test_cases {
        let patterns = detector.detect(input);
        let detected_ports: Vec<u16> = patterns
            .iter()
            .filter_map(|p| match p {
                DetectedPattern::Port(port) => Some(*port),
                _ => None,
            })
            .collect();
        
        assert_eq!(detected_ports, expected_ports, "Failed for input: {}", input);
    }
}

#[test]
fn test_url_detection() {
    let detector = PatternDetector::new();
    
    let test_cases = vec![
        ("Visit http://example.com", vec!["http://example.com"]),
        ("API at https://api.example.com/v1", vec!["https://api.example.com/v1"]),
        ("Multiple URLs: http://localhost:8080 and https://google.com", 
         vec!["http://localhost:8080", "https://google.com"]),
        ("No URLs in this text", vec![]),
        ("Server running at http://0.0.0.0:3000/", vec!["http://0.0.0.0:3000/"]),
    ];
    
    for (input, expected_urls) in test_cases {
        let patterns = detector.detect(input);
        let detected_urls: Vec<&str> = patterns
            .iter()
            .filter_map(|p| match p {
                DetectedPattern::Url(url) => Some(url.as_str()),
                _ => None,
            })
            .collect();
        
        assert_eq!(detected_urls, expected_urls, "Failed for input: {}", input);
    }
}

#[test]
fn test_error_detection() {
    let detector = PatternDetector::new();
    
    let test_cases = vec![
        ("ERROR: Connection failed", true),
        ("Error in module X", true),
        ("Failed to connect to database", true),
        ("FATAL: System crash", true),
        ("panic: runtime error", true),
        ("Everything is working fine", false),
        ("Successful operation", false),
        ("WARN: This is just a warning", false),
    ];
    
    for (input, should_detect) in test_cases {
        let patterns = detector.detect(input);
        let has_error = patterns.iter().any(|p| matches!(p, DetectedPattern::Error(_)));
        
        assert_eq!(has_error, should_detect, "Failed for input: {}", input);
    }
}

#[test]
fn test_file_path_detection() {
    let detector = PatternDetector::new();
    
    let test_cases = vec![
        ("Loading config from /etc/app/config.json", vec!["/etc/app/config.json"]),
        ("File not found: /home/user/data.txt", vec!["/home/user/data.txt"]),
        ("Reading ./relative/path.yml", vec!["./relative/path.yml"]),
        ("Multiple files: /tmp/a.txt and /var/log/app.log", vec!["/tmp/a.txt", "/var/log/app.log"]),
        ("No file paths here", vec![]),
    ];
    
    for (input, expected_paths) in test_cases {
        let patterns = detector.detect(input);
        let detected_paths: Vec<&str> = patterns
            .iter()
            .filter_map(|p| match p {
                DetectedPattern::FilePath(path) => Some(path.as_str()),
                _ => None,
            })
            .collect();
        
        assert_eq!(detected_paths.len(), expected_paths.len(), 
                   "Failed count for input: {}", input);
        
        for expected in expected_paths {
            assert!(detected_paths.iter().any(|&p| p == expected),
                    "Missing path {} in input: {}", expected, input);
        }
    }
}

#[test]
fn test_multiple_patterns_in_line() {
    let detector = PatternDetector::new();
    let line = "ERROR: Failed to connect to http://localhost:8080, check /var/log/app.log";
    let patterns = detector.detect(line);
    
    // Should detect error, URL, port, and file path
    let has_error = patterns.iter().any(|p| matches!(p, DetectedPattern::Error(_)));
    let has_url = patterns.iter().any(|p| matches!(p, DetectedPattern::Url(_)));
    let has_port = patterns.iter().any(|p| matches!(p, DetectedPattern::Port(_)));
    let has_file = patterns.iter().any(|p| matches!(p, DetectedPattern::FilePath(_)));
    
    assert!(has_error);
    assert!(has_url);
    assert!(has_port);
    assert!(has_file);
}

#[test]
fn test_key_event_detection() {
    let detector = PatternDetector::new();
    
    let key_events = vec![
        "Deployment complete",
        "Build finished",
        "Ready to accept connections",
        "Server started successfully",
    ];
    
    for event in key_events {
        let patterns = detector.detect(event);
        let has_key_event = patterns.iter().any(|p| matches!(p, DetectedPattern::KeyEvent(_)));
        assert!(has_key_event, "Should detect key event: {}", event);
    }
}

#[test]
fn test_ip_address_detection() {
    let detector = PatternDetector::new();
    
    let test_cases = vec![
        ("Connecting to 192.168.1.1", "192.168.1.1"),
        ("Server at 10.0.0.1", "10.0.0.1"),
        ("Bound to 127.0.0.1", "127.0.0.1"),
    ];
    
    for (input, expected_ip) in test_cases {
        let patterns = detector.detect(input);
        let found = patterns.iter().any(|p| match p {
            DetectedPattern::IpAddress(ip) => ip == expected_ip,
            _ => false,
        });
        assert!(found, "Should find IP {} in: {}", expected_ip, input);
    }
}

// Property-based tests
proptest! {
    #[test]
    fn prop_port_always_in_valid_range(port in 1u16..=65535) {
        let detector = PatternDetector::new();
        let line = format!("Server on port {}", port);
        let patterns = detector.detect(&line);
        
        let detected_port = patterns.iter()
            .find_map(|p| match p {
                DetectedPattern::Port(p) => Some(*p),
                _ => None,
            });
        
        assert_eq!(detected_port, Some(port));
    }
    
    #[test]
    fn prop_url_with_random_domains(domain in "[a-z]{3,10}\\.[a-z]{2,5}") {
        let detector = PatternDetector::new();
        let url = format!("http://{}", domain);
        let line = format!("Visit {}", url);
        let patterns = detector.detect(&line);
        
        let has_url = patterns.iter().any(|p| match p {
            DetectedPattern::Url(u) => u == &url,
            _ => false,
        });
        
        assert!(has_url);
    }
    
    #[test]
    fn prop_no_false_patterns_in_random_text(s in "[a-zA-Z ]{20,100}") {
        let detector = PatternDetector::new();
        // Random alphabetic text shouldn't trigger patterns (unless it contains keywords)
        if !s.contains("error") && !s.contains("Error") && !s.contains("ERROR") &&
           !s.contains("fail") && !s.contains("Fail") && !s.contains("FAIL") &&
           !s.contains("port") && !s.contains("Port") && !s.contains("complete") {
            let patterns = detector.detect(&s);
            // Should only potentially have KeyEvent patterns
            for pattern in &patterns {
                assert!(matches!(pattern, DetectedPattern::KeyEvent(_)),
                        "Found unexpected pattern in random text: {:?}", pattern);
            }
        }
    }
}

#[test]
fn test_pattern_extraction_performance() {
    use std::time::Instant;
    
    let detector = PatternDetector::new();
    
    // Create a large log line with multiple patterns
    let mut large_line = String::with_capacity(10000);
    for i in 0..100 {
        large_line.push_str(&format!(
            "Server {} on port {} at http://server{}.com/api ",
            i, 8000 + i, i
        ));
    }
    
    let start = Instant::now();
    let patterns = detector.detect(&large_line);
    let duration = start.elapsed();
    
    // Should complete quickly even for large lines
    assert!(duration.as_millis() < 100, "Pattern detection took too long: {:?}", duration);
    
    // Should find patterns
    assert!(!patterns.is_empty());
}