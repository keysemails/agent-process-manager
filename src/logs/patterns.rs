//! Pattern detection for common log patterns

use regex::Regex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum DetectedPattern {
    Url(String),
    Port(u16),
    IpAddress(String),
    BuildTime(f64),
    Error(String),
    ErrorPattern(String),
    KeyEvent(String),
    FilePath(String),
    EmailAddress(String),
}

pub struct PatternDetector {
    patterns: Vec<CompiledPattern>,
}

struct CompiledPattern {
    #[allow(dead_code)]
    name: &'static str,
    regex: Regex,
    extractor: Box<dyn Fn(&regex::Captures) -> Option<DetectedPattern> + Send + Sync>,
}

impl PatternDetector {
    pub fn new() -> Self {
        let patterns = vec![
            CompiledPattern {
                name: "url",
                regex: Regex::new(r"https?://[^\s]+").unwrap(),
                extractor: Box::new(|caps| {
                    caps.get(0).map(|m| DetectedPattern::Url(m.as_str().to_string()))
                }),
            },
            CompiledPattern {
                name: "port",
                // Primary port detection - explicit port contexts and URLs
                regex: Regex::new(r"(?i)(?:(?:listening\s+on\s+)(?:\d{1,3}\.){3}\d{1,3}:(\d{1,5})\b|(?:(?:0\.0\.0\.0|localhost|127\.0\.0\.1|::1|::)|https?://[^:]+):(\d{1,5})\b|(?:port[s]?\s*[:=]?\s*|listening\s+on\s*:?\s*|started\s+on\s+)(\d{1,5})\b)").unwrap(),
                extractor: Box::new(|caps| {
                    // Try group 1 first (IP:port), then group 2 (URL/IP contexts), then group 3 (port contexts)
                    caps.get(1).or_else(|| caps.get(2)).or_else(|| caps.get(3))
                        .and_then(|m| m.as_str().parse::<u16>().ok())
                        .filter(|&p| p > 0)
                        .map(DetectedPattern::Port)
                }),
            },
            CompiledPattern {
                name: "port_secondary",
                // Secondary port detection - "at/on" followed by a number
                // But exclude timestamps by checking context
                regex: Regex::new(r"(?i)\b(?:at|on)\s+(\d{3,5})\b").unwrap(),
                extractor: Box::new(|caps| {
                    caps.get(1)
                        .and_then(|m| m.as_str().parse::<u16>().ok())
                        .filter(|&p| p >= 100)  // Ports below 100 are rare, helps avoid false positives
                        .map(DetectedPattern::Port)
                }),
            },
            CompiledPattern {
                name: "ip_address",
                regex: Regex::new(r"\b(?:\d{1,3}\.){3}\d{1,3}\b").unwrap(),
                extractor: Box::new(|caps| {
                    caps.get(0).map(|m| DetectedPattern::IpAddress(m.as_str().to_string()))
                }),
            },
            CompiledPattern {
                name: "build_time",
                regex: Regex::new(r"(?i)(?:built?|compiled?|finished?)\s+in\s+(\d+\.?\d*)\s*(?:s|seconds?|ms|milliseconds?)").unwrap(),
                extractor: Box::new(|caps| {
                    caps.get(1)
                        .and_then(|m| m.as_str().parse::<f64>().ok())
                        .map(DetectedPattern::BuildTime)
                }),
            },
            CompiledPattern {
                name: "error",
                regex: Regex::new(r"(?i)(?:error|exception|failed|fatal|panic)(?::?\s*(.+))?").unwrap(),
                extractor: Box::new(|caps| {
                    let msg = caps.get(1)
                        .map(|m| m.as_str().to_string())
                        .unwrap_or_else(|| caps.get(0).unwrap().as_str().to_string());
                    Some(DetectedPattern::Error(msg))
                }),
            },
            CompiledPattern {
                name: "file_path",
                regex: Regex::new(r"(?:/[\w\-./]+(?:\.\w+)?|\.{1,2}/[\w\-./]+(?:\.\w+)?)").unwrap(),
                extractor: Box::new(|caps| {
                    caps.get(0).map(|m| DetectedPattern::FilePath(m.as_str().to_string()))
                }),
            },
        ];

        // Key event patterns - these are important status messages
        let _key_event_patterns = vec![
            Regex::new(r"(?i)server\s+started").unwrap(),
            Regex::new(r"(?i)listening\s+on").unwrap(),
            Regex::new(r"(?i)connected\s+to\s+database").unwrap(),
            Regex::new(r"(?i)compilation\s+(?:complete|succeeded|finished)").unwrap(),
            Regex::new(r"(?i)deployment\s+(?:complete|succeeded|finished)").unwrap(),
            Regex::new(r"(?i)build\s+(?:complete|succeeded|finished)").unwrap(),
            Regex::new(r"(?i)ready\s+to\s+accept\s+connections").unwrap(),
        ];

        Self { patterns }
    }

    pub fn detect(&self, line: &str) -> Vec<DetectedPattern> {
        let mut detected = Vec::new();
        let mut seen_ports = std::collections::HashSet::new();

        for pattern in &self.patterns {
            // For patterns that can occur multiple times, find all matches
            for captures in pattern.regex.captures_iter(line) {
                if let Some(result) = (pattern.extractor)(&captures) {
                    // Deduplicate port detections
                    match &result {
                        DetectedPattern::Port(port) => {
                            if seen_ports.insert(*port) {
                                detected.push(result);
                            }
                        }
                        _ => detected.push(result),
                    }
                }
            }
        }

        // Check for key events
        if self.is_key_event(line) {
            detected.push(DetectedPattern::KeyEvent(line.to_string()));
        }

        detected
    }

    fn is_key_event(&self, line: &str) -> bool {
        // Simple heuristic for now
        let lower = line.to_lowercase();
        lower.contains("started") ||
        lower.contains("listening on") ||
        lower.contains("connected to") ||
        lower.contains("compilation complete") ||
        lower.contains("build succeeded") ||
        lower.contains("build finished") ||
        lower.contains("deployment complete") ||
        lower.contains("ready") ||
        lower.contains("complete") ||
        lower.contains("finished")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_port_detection() {
        let detector = PatternDetector::new();
        
        let test_cases = vec![
            ("Server listening on port 3000", Some(3000)),
            ("Listening on :8080", Some(8080)),
            ("Server started on 5000", Some(5000)),
            ("Random text without port", None),
            // Test that timestamps are NOT detected as ports
            ("[11:41:22] Processing request", None),
            ("[10:56:09] Starting server", None),
            ("Time: 12:34:56", None),
            // Test valid port patterns still work
            ("http://localhost:8080", Some(8080)),
            ("0.0.0.0:3000", Some(3000)),
            ("listening on port 80", Some(80)),
            ("Port: 443", Some(443)),
            ("on port 9000", Some(9000)),
        ];

        for (input, expected_port) in test_cases {
            let patterns = detector.detect(input);
            let found_port = patterns.iter().find_map(|p| {
                if let DetectedPattern::Port(port) = p {
                    Some(*port)
                } else {
                    None
                }
            });
            assert_eq!(found_port, expected_port, "Failed for: {}", input);
        }
    }

    #[test]
    fn test_url_detection() {
        let detector = PatternDetector::new();
        let patterns = detector.detect("Server running at http://localhost:3000");
        
        assert!(patterns.iter().any(|p| matches!(p, DetectedPattern::Url(url) if url == "http://localhost:3000")));
    }

    #[test]
    fn test_timestamp_patterns() {
        let detector = PatternDetector::new();
        
        // Test various timestamp formats that should NOT detect ports
        let timestamp_cases = vec![
            "[11:41:22] Processing request",
            "[10:56:09] Starting server",
            "Time: 12:34:56",
            "2024-01-01 12:34:56 INFO Server started",
            "[2024-01-01 12:34:56] INFO: Application ready",
            "12:34:56.789 Debug message",
            "[12:34] Short time format",
        ];
        
        for input in timestamp_cases {
            let patterns = detector.detect(input);
            let ports: Vec<u16> = patterns.iter().filter_map(|p| {
                if let DetectedPattern::Port(port) = p {
                    Some(*port)
                } else {
                    None
                }
            }).collect();
            
            assert!(ports.is_empty(), 
                "Timestamp '{}' incorrectly detected ports: {:?}", input, ports);
        }
    }
    
    #[test]
    fn test_specific_timestamp_issue() {
        let detector = PatternDetector::new();
        
        // Test various cases that might trigger the bug
        let test_cases = vec![
            ("[11:41:22] Processing request", false),
            ("11:41:22 Processing request", false),
            ("At 11:41:22 server started", false),
            ("Started at 11:41:22", false),
            ("Time is 11:41:22", false),
            ("11:41:22.123 Debug log", false),
            ("at 11:41:22", false),  // "at" followed by timestamp
            ("Started at 8080", true),  // "at" followed by port number SHOULD detect
            ("localhost:41 running", true),  // This SHOULD detect port 41
            ("Listening on :8080", true),  // Should detect 8080
            ("Server started on 3000", true),  // Should detect 3000
        ];
        
        for (input, should_detect) in test_cases {
            let patterns = detector.detect(input);
            let ports: Vec<u16> = patterns.iter().filter_map(|p| {
                if let DetectedPattern::Port(port) = p {
                    Some(*port)
                } else {
                    None
                }
            }).collect();
            
            println!("\nTesting: {} (should detect: {})", input, should_detect);
            for port in &ports {
                println!("  Found port: {}", port);
            }
            
            if should_detect {
                assert!(!ports.is_empty(), 
                    "Failed to detect port in '{}'", input);
            } else {
                assert!(ports.is_empty(), 
                    "Incorrectly detected ports {:?} in '{}'", ports, input);
            }
        }
    }
}