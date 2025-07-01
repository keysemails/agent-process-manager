//! Log management module

mod storage;
mod patterns;
mod summary;

pub use storage::{LogStorage, LogEntry, LogLevel, LogQuery, LogFormat, LogSummary, ProcessRecord};
pub use patterns::{PatternDetector, DetectedPattern};
pub use summary::LogSummarizer;