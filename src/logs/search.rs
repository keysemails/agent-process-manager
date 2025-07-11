//! Full-text search implementation using Tantivy
//! 
//! This module provides powerful search capabilities for logs with:
//! - Boolean queries (AND, OR, NOT)
//! - Fuzzy search with typo tolerance  
//! - Proximity search
//! - Range queries for timestamps
//! - Faceted search by level, process, patterns
//! - Highlighted snippets

use crate::{Result, ApmError};
use crate::logs::storage::{LogEntry, LogLevel};
use crate::logs::patterns::DetectedPattern;
use crate::process::ProcessId;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tantivy::{
    collector::{Count, TopDocs},
    directory::MmapDirectory,
    query::{BooleanQuery, Occur, Query, QueryParser, TermQuery},
    schema::{Field, Schema, TextFieldIndexing, TextOptions, Value, FAST, STORED, TEXT},
    DateTime as TantivyDateTime,
    doc, Index, IndexReader, IndexWriter, Term, TantivyDocument,
};
use tokio::sync::Mutex;

/// Search query with various filter options
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchQuery {
    /// Query string using Lucene-compatible syntax
    pub query: String,
    /// Filter by specific process ID
    pub process_id: Option<ProcessId>,
    /// Filter by log level
    pub level: Option<LogLevel>,
    /// Filter by time range (start)
    pub since: Option<DateTime<Utc>>,
    /// Filter by time range (end)
    pub until: Option<DateTime<Utc>>,
    /// Filter by detected patterns
    pub patterns: Option<Vec<String>>,
    /// Maximum number of results
    pub limit: Option<usize>,
    /// Offset for pagination
    pub offset: Option<usize>,
    /// Whether to include highlighted snippets
    pub highlight: Option<bool>,
}

/// Search result with relevance score and optional highlighting
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    /// Reference to the original log entry
    pub log_id: i64,
    /// Relevance score (0.0 to 1.0)
    pub score: f32,
    /// Highlighted snippet with search terms emphasized
    pub snippet: Option<String>,
    /// Process ID
    pub process_id: ProcessId,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// Log level
    pub level: LogLevel,
    /// Raw log line
    pub raw_line: String,
    /// Clean log line (without ANSI codes)
    pub clean_line: String,
    /// Detected patterns
    pub patterns: Vec<DetectedPattern>,
}

/// Faceted search results showing count of results by category
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchFacets {
    /// Count by log level
    pub level: std::collections::HashMap<String, u64>,
    /// Count by process
    pub process: std::collections::HashMap<String, u64>,
    /// Count by pattern type
    pub patterns: std::collections::HashMap<String, u64>,
}

/// Complete search response with results, facets, and metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResponse {
    /// Search results
    pub results: Vec<SearchResult>,
    /// Total number of matching documents
    pub total_hits: u64,
    /// Query execution time in milliseconds
    pub query_time_ms: u64,
    /// Faceted results
    pub facets: Option<SearchFacets>,
    /// Query that was executed
    pub query: String,
}

/// Tantivy schema fields for log entries
#[derive(Clone)]
pub struct LogSchema {
    /// Log ID (reference to SQLite)
    pub log_id: Field,
    /// Process ID
    pub process_id: Field,
    /// Log message (full-text searchable)
    pub message: Field,
    /// Clean message without ANSI codes
    pub clean_message: Field,
    /// Log level (faceted)
    pub level: Field,
    /// Timestamp (range queryable)
    pub timestamp: Field,
    /// Detected patterns (hierarchical facets)
    pub patterns: Field,
    /// Full document schema
    pub schema: Schema,
}

impl LogSchema {
    pub fn new() -> Self {
        let mut schema_builder = Schema::builder();

        // Log ID - stored for reference back to SQLite
        let log_id = schema_builder.add_i64_field("log_id", STORED);

        // Process ID - stored and indexed for exact matching (no tokenization)
        let process_id_options = TextOptions::default()
            .set_indexing_options(
                TextFieldIndexing::default()
                    .set_tokenizer("raw")  // Use raw tokenizer for exact matching
                    .set_index_option(tantivy::schema::IndexRecordOption::Basic)
            )
            .set_stored();
        let process_id = schema_builder.add_text_field("process_id", process_id_options);

        // Message - full-text searchable with positions for highlighting
        let message_indexing = TextFieldIndexing::default()
            .set_tokenizer("en_stem")
            .set_index_option(tantivy::schema::IndexRecordOption::WithFreqsAndPositions);
        let message_options = TextOptions::default()
            .set_indexing_options(message_indexing)
            .set_stored();
        let message = schema_builder.add_text_field("message", message_options);

        // Clean message - searchable version without ANSI codes
        let clean_message = schema_builder.add_text_field("clean_message", TEXT | STORED);

        // Level - fast field for faceting and filtering
        let level = schema_builder.add_text_field("level", TEXT | FAST | STORED);

        // Timestamp - fast field for range queries
        let timestamp = schema_builder.add_date_field("timestamp", FAST | STORED);

        // Patterns - faceted field for pattern-based filtering
        let patterns = schema_builder.add_facet_field("patterns", STORED);

        let schema = schema_builder.build();

        Self {
            log_id,
            process_id,
            message,
            clean_message,
            level,
            timestamp,
            patterns,
            schema,
        }
    }
}

/// High-performance search engine for log entries
pub struct LogSearchEngine {
    /// Tantivy index
    index: Index,
    /// Schema definition
    schema: LogSchema,
    /// Index writer (single writer, thread-safe)
    writer: Arc<Mutex<IndexWriter>>,
    /// Index reader for searching
    reader: IndexReader,
    /// Query parser for Lucene-compatible syntax
    query_parser: QueryParser,
    /// Index directory path
    index_path: PathBuf,
}

impl LogSearchEngine {
    /// Create a new search engine with the specified index directory
    pub async fn new<P: AsRef<Path>>(index_path: P) -> Result<Self> {
        Self::new_with_config(index_path, 50).await
    }
    
    /// Create a new search engine with custom buffer size
    pub async fn new_with_config<P: AsRef<Path>>(index_path: P, buffer_size_mb: usize) -> Result<Self> {
        let index_path = index_path.as_ref().to_path_buf();
        
        // Create index directory if it doesn't exist
        if !index_path.exists() {
            std::fs::create_dir_all(&index_path)
                .map_err(|e| ApmError::IoError(format!("Failed to create index directory: {}", e)))?;
        }

        let schema = LogSchema::new();
        
        // Open or create the index
        let directory = MmapDirectory::open(&index_path)
            .map_err(|e| ApmError::IoError(format!("Failed to open index directory: {}", e)))?;
        
        let index = Index::open_or_create(directory, schema.schema.clone())
            .map_err(|e| ApmError::SearchError(format!("Failed to create index: {}", e)))?;
        
        // Register the raw tokenizer for exact matching
        index.tokenizers().register("raw", tantivy::tokenizer::RawTokenizer::default());

        // Create writer with configured buffer size
        let buffer_bytes = buffer_size_mb * 1_000_000;
        let writer = index
            .writer(buffer_bytes)
            .map_err(|e| ApmError::SearchError(format!("Failed to create index writer: {}", e)))?;

        // Create reader
        let reader = index
            .reader()
            .map_err(|e| ApmError::SearchError(format!("Failed to create index reader: {}", e)))?;

        // Create query parser for natural language queries
        let query_parser = QueryParser::for_index(
            &index,
            vec![schema.message, schema.clean_message, schema.process_id],
        );

        Ok(Self {
            index,
            schema,
            writer: Arc::new(Mutex::new(writer)),
            reader,
            query_parser,
            index_path,
        })
    }

    /// Index a single log entry
    pub async fn index_log(&self, log: &LogEntry) -> Result<()> {
        let mut doc = TantivyDocument::new();

        // Add fields to document
        doc.add_i64(self.schema.log_id, log.id);
        doc.add_text(self.schema.process_id, log.process_id.to_string());
        doc.add_text(self.schema.message, &log.raw_line);
        doc.add_text(self.schema.clean_message, &log.clean_line);
        doc.add_text(self.schema.level, &format!("{:?}", log.level));
        
        // Convert timestamp to Tantivy format
        let tantivy_timestamp = TantivyDateTime::from_timestamp_millis(log.timestamp.timestamp_millis());
        doc.add_date(self.schema.timestamp, tantivy_timestamp);

        // Add patterns as facets
        for pattern in &log.patterns {
            let facet_path = match pattern {
                DetectedPattern::Port(port) => format!("/pattern/port/{}", port),
                DetectedPattern::Url(url) => format!("/pattern/url/{}", url),
                DetectedPattern::FilePath(path) => format!("/pattern/file/{}", path),
                DetectedPattern::ErrorPattern(error) => format!("/pattern/error/{}", error),
                DetectedPattern::Error(error) => format!("/pattern/error/{}", error),
                DetectedPattern::KeyEvent(event) => format!("/pattern/event/{}", event),
                DetectedPattern::IpAddress(ip) => format!("/pattern/ip/{}", ip),
                DetectedPattern::EmailAddress(email) => format!("/pattern/email/{}", email),
                DetectedPattern::BuildTime(time) => format!("/pattern/buildtime/{}", time),
            };
            
            if let Ok(facet) = tantivy::schema::Facet::from_text(&facet_path) {
                doc.add_facet(self.schema.patterns, facet);
            }
        }

        // Add document to index
        let writer = self.writer.lock().await;
        writer
            .add_document(doc)
            .map_err(|e| ApmError::SearchError(format!("Failed to add document: {}", e)))?;

        Ok(())
    }

    /// Index multiple log entries in batch for better performance
    pub async fn index_logs_batch(&self, logs: &[LogEntry]) -> Result<()> {
        let writer = self.writer.lock().await;
        
        for log in logs {
            let mut doc = TantivyDocument::new();

            doc.add_i64(self.schema.log_id, log.id);
            doc.add_text(self.schema.process_id, log.process_id.to_string());
            doc.add_text(self.schema.message, &log.raw_line);
            doc.add_text(self.schema.clean_message, &log.clean_line);
            doc.add_text(self.schema.level, &format!("{:?}", log.level));
            
            let tantivy_timestamp = TantivyDateTime::from_timestamp_millis(log.timestamp.timestamp_millis());
            doc.add_date(self.schema.timestamp, tantivy_timestamp);

            for pattern in &log.patterns {
                let facet_path = match pattern {
                    DetectedPattern::Port(port) => format!("/pattern/port/{}", port),
                    DetectedPattern::Url(url) => format!("/pattern/url/{}", url),
                    DetectedPattern::FilePath(path) => format!("/pattern/file/{}", path),
                    DetectedPattern::ErrorPattern(error) => format!("/pattern/error/{}", error),
                    DetectedPattern::Error(error) => format!("/pattern/error/{}", error),
                    DetectedPattern::KeyEvent(event) => format!("/pattern/event/{}", event),
                    DetectedPattern::IpAddress(ip) => format!("/pattern/ip/{}", ip),
                    DetectedPattern::EmailAddress(email) => format!("/pattern/email/{}", email),
                    DetectedPattern::BuildTime(time) => format!("/pattern/buildtime/{}", time),
                };
                
                if let Ok(facet) = tantivy::schema::Facet::from_text(&facet_path) {
                    doc.add_facet(self.schema.patterns, facet);
                }
            }

            writer
                .add_document(doc)
                .map_err(|e| ApmError::SearchError(format!("Failed to add document: {}", e)))?;
        }

        Ok(())
    }

    /// Commit all pending changes to make them searchable
    pub async fn commit(&self) -> Result<()> {
        let mut writer = self.writer.lock().await;
        writer
            .commit()
            .map_err(|e| ApmError::SearchError(format!("Failed to commit index: {}", e)))?;
        
        // Reload reader to see new changes
        self.reader
            .reload()
            .map_err(|e| ApmError::SearchError(format!("Failed to reload reader: {}", e)))?;

        Ok(())
    }

    /// Search logs with the given query
    pub async fn search(&self, query: SearchQuery) -> Result<SearchResponse> {
        let start_time = std::time::Instant::now();
        
        // Parse the main query
        let parsed_query = self
            .query_parser
            .parse_query(&query.query)
            .map_err(|e| ApmError::SearchError(format!("Failed to parse query: {}", e)))?;

        // Build the final query with filters
        let final_query: Box<dyn Query> = if let Some(ref process_id) = query.process_id {
            // Create a boolean query that combines the text query with process_id filter
            let process_term = Term::from_field_text(self.schema.process_id, &process_id.to_string());
            let process_query = TermQuery::new(process_term, Default::default());
            
            Box::new(BooleanQuery::new(vec![
                (Occur::Must, parsed_query),
                (Occur::Must, Box::new(process_query)),
            ]))
        } else {
            parsed_query
        };

        // Execute search
        let searcher = self.reader.searcher();
        let limit = query.limit.unwrap_or(50).min(1000); // Cap at 1000
        let offset = query.offset.unwrap_or(0);
        
        let top_docs = searcher
            .search(&final_query, &TopDocs::with_limit(limit + offset))
            .map_err(|e| ApmError::SearchError(format!("Search failed: {}", e)))?;

        // Get total count
        let total_hits = searcher
            .search(&final_query, &Count)
            .map_err(|e| ApmError::SearchError(format!("Count failed: {}", e)))?;

        // Convert results
        let mut results = Vec::new();
        for (_score, doc_address) in top_docs.into_iter().skip(offset).take(limit) {
            if let Ok(doc) = searcher.doc(doc_address) {
                if let Some(result) = self.document_to_search_result(doc, _score) {
                    results.push(result);
                }
            }
        }

        let query_time_ms = start_time.elapsed().as_millis() as u64;

        Ok(SearchResponse {
            results,
            total_hits: total_hits as u64,
            query_time_ms,
            facets: None, // TODO: Implement faceted search
            query: query.query,
        })
    }

    /// Convert Tantivy document to SearchResult
    fn document_to_search_result(&self, doc: TantivyDocument, score: f32) -> Option<SearchResult> {
        // Extract fields from document
        let log_id = doc.get_first(self.schema.log_id)?.as_i64()?;
        let process_id_str = doc.get_first(self.schema.process_id)?.as_str()?;
        let process_id = ProcessId(process_id_str.parse().ok()?);
        let raw_line = doc.get_first(self.schema.message)?.as_str()?.to_string();
        let clean_line = doc.get_first(self.schema.clean_message)?.as_str()?.to_string();
        let level_str = doc.get_first(self.schema.level)?.as_str()?;
        let timestamp_millis = doc.get_first(self.schema.timestamp)?.as_datetime()?.into_timestamp_millis();
        let timestamp = DateTime::from_timestamp_millis(timestamp_millis)?;

        // Parse level
        let level = match level_str {
            "Error" => LogLevel::Error,
            "Warn" => LogLevel::Warn,
            "Debug" => LogLevel::Debug,
            _ => LogLevel::Info,
        };

        // TODO: Extract patterns from facets
        let patterns = vec![];

        Some(SearchResult {
            log_id,
            score,
            snippet: None, // TODO: Implement highlighting
            process_id,
            timestamp,
            level,
            raw_line,
            clean_line,
            patterns,
        })
    }

    /// Get index statistics
    pub async fn get_stats(&self) -> Result<IndexStats> {
        let searcher = self.reader.searcher();
        let num_docs = searcher.num_docs();
        
        // Calculate index size
        let index_size = std::fs::metadata(&self.index_path)
            .map(|m| m.len())
            .unwrap_or(0);

        Ok(IndexStats {
            num_docs,
            index_size_bytes: index_size,
            index_path: self.index_path.clone(),
        })
    }

    /// Delete all documents for a specific process
    pub async fn delete_process_logs(&self, process_id: &ProcessId) -> Result<()> {
        let mut writer = self.writer.lock().await;
        let term = Term::from_field_text(self.schema.process_id, &process_id.to_string());
        writer.delete_term(term);
        Ok(())
    }

    /// Optimize the index for better search performance
    pub async fn optimize(&self) -> Result<()> {
        // In Tantivy 0.22, optimization happens automatically
        // We can just commit to ensure all changes are persisted
        self.commit().await
    }
}

/// Index statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexStats {
    /// Number of documents in the index
    pub num_docs: u64,
    /// Index size in bytes
    pub index_size_bytes: u64,
    /// Path to index directory
    pub index_path: PathBuf,
}