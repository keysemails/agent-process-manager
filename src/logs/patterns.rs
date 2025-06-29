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
    KeyEvent(String),
    FilePath(String),
}

pub struct PatternDetector {
    patterns: Vec<CompiledPattern>,
}

struct CompiledPattern {
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
                regex: Regex::new(r"(?i)(?:ports?\s+|port:?\s*|on\s+port\s*|on\s+|:\s*)(\d{1,5})\b").unwrap(),
                extractor: Box::new(|caps| {
                    caps.get(1)
                        .and_then(|m| m.as_str().parse::<u16>().ok())
                        .filter(|&p| p > 0 && p <= 65535)
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

        for pattern in &self.patterns {
            // For patterns that can occur multiple times, find all matches
            for captures in pattern.regex.captures_iter(line) {
                if let Some(result) = (pattern.extractor)(&captures) {
                    detected.push(result);
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
}