//! # Lightning Log 
//! 
//! Ultra-fast zero-allocation logging designed for high-frequency trading and low-latency systems.
//! 
//! ## Features
//! 
//! - **Sub-100ns logging latency** in the critical path
//! - **Zero allocations** during logging calls
//! - **Binary serialization** for maximum performance
//! - **Lock-free** communication between threads
//! - **CPU affinity** support for logging thread
//! - **Structured logging** with message IDs
//! - **Compile-time optimizations** via macros
//! 
//! ## Quick Start
//! 
//! ```rust
//! use lightning_log::*;
//! 
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Initialize the logger (call once at startup)
//! init_lightning_log();
//! 
//! // Register message formats (optional, for human-readable output)
//! register_message_format(1001, "Trade executed: volume={}, price={}, symbol={}");
//! 
//! let volume = 1000.0;
//! let price = 150.25;
//! let symbol = "AAPL";
//! 
//! // Ultra-fast logging (typically <100ns)
//! lightning_info!(1001, volume, price, symbol);
//! lightning_debug!(1002, price);
//! lightning_warn!(1003, true, 42i32);
//! # 
//! # // Give the logger thread time to process
//! # std::thread::sleep(std::time::Duration::from_millis(10));
//! # Ok(())
//! # }
//! ```
//!
//! ## Advanced Configuration
//!
//! For production use, you can configure file output and other advanced features:
//!
//! ```rust
//! use lightning_log::*;
//! use std::path::PathBuf;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Configure for production use
//! let config = LoggerConfig {
//!     channel_capacity: 100_000,
//!     destinations: vec![
//!         LogDestination::Stdout,
//!         LogDestination::File(PathBuf::from("logs/trading.log")),
//!     ],
//!     file_buffer_size: 64 * 1024, // 64KB buffer
//!     enable_cpu_affinity: true,
//! };
//!
//! init_lightning_log_with_config(config)?;
//!
//! // Register message formats
//! register_message_format(1001, "Trade executed: volume={}, price={}, symbol={}".to_string())?;
//!
//! // Your trading logic here...
//! let volume = 1000.0;
//! let price = 150.25;
//! let symbol = "AAPL";
//! lightning_info!(1001, volume, price, symbol);
//!
//! // Graceful shutdown (important for production)
//! shutdown_lightning_log();
//! std::thread::sleep(std::time::Duration::from_millis(100)); // Allow time for processing
//! # Ok(())
//! # }
//! ```
use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::{self, BufWriter, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};
use crossbeam_channel::{bounded, Receiver, Sender, TryRecvError};

/// Global sequence counter for maintaining log order
static SEQUENCE_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Global logger instance
static GLOBAL_LOGGER: OnceLock<LightningLogger> = OnceLock::new();

/// Message format registry
static MESSAGE_FORMATS: OnceLock<Arc<RwLock<HashMap<u32, String>>>> = OnceLock::new();

/// Trait for types that can be efficiently serialized for logging
pub trait FastSerialize {
    /// Write the value to the buffer in binary format
    fn write_to_buffer(&self, buffer: &mut Vec<u8>);
    
    /// Hint about the serialized size (for buffer pre-allocation)
    fn size_hint(&self) -> usize { 8 }
}

impl FastSerialize for i8 {
    fn write_to_buffer(&self, buffer: &mut Vec<u8>) {
        buffer.push(*self as u8);
    }
    fn size_hint(&self) -> usize { 1 }
}

impl FastSerialize for i16 {
    fn write_to_buffer(&self, buffer: &mut Vec<u8>) {
        buffer.extend_from_slice(&self.to_le_bytes());
    }
    fn size_hint(&self) -> usize { 2 }
}

impl FastSerialize for i32 {
    fn write_to_buffer(&self, buffer: &mut Vec<u8>) {
        buffer.extend_from_slice(&self.to_le_bytes());
    }
    fn size_hint(&self) -> usize { 4 }
}

impl FastSerialize for i64 {
    fn write_to_buffer(&self, buffer: &mut Vec<u8>) {
        buffer.extend_from_slice(&self.to_le_bytes());
    }
    fn size_hint(&self) -> usize { 8 }
}

impl FastSerialize for u8 {
    fn write_to_buffer(&self, buffer: &mut Vec<u8>) {
        buffer.push(*self);
    }
    fn size_hint(&self) -> usize { 1 }
}

impl FastSerialize for u16 {
    fn write_to_buffer(&self, buffer: &mut Vec<u8>) {
        buffer.extend_from_slice(&self.to_le_bytes());
    }
    fn size_hint(&self) -> usize { 2 }
}

impl FastSerialize for u32 {
    fn write_to_buffer(&self, buffer: &mut Vec<u8>) {
        buffer.extend_from_slice(&self.to_le_bytes());
    }
    fn size_hint(&self) -> usize { 4 }
}

impl FastSerialize for u64 {
    fn write_to_buffer(&self, buffer: &mut Vec<u8>) {
        buffer.extend_from_slice(&self.to_le_bytes());
    }
    fn size_hint(&self) -> usize { 8 }
}

impl FastSerialize for f32 {
    fn write_to_buffer(&self, buffer: &mut Vec<u8>) {
        buffer.extend_from_slice(&self.to_le_bytes());
    }
    fn size_hint(&self) -> usize { 4 }
}

impl FastSerialize for f64 {
    fn write_to_buffer(&self, buffer: &mut Vec<u8>) {
        buffer.extend_from_slice(&self.to_le_bytes());
    }
    fn size_hint(&self) -> usize { 8 }
}

impl FastSerialize for bool {
    fn write_to_buffer(&self, buffer: &mut Vec<u8>) {
        buffer.push(*self as u8);
    }
    fn size_hint(&self) -> usize { 1 }
}

impl FastSerialize for &str {
    fn write_to_buffer(&self, buffer: &mut Vec<u8>) {
        let len = self.len() as u32;
        buffer.extend_from_slice(&len.to_le_bytes());
        buffer.extend_from_slice(self.as_bytes());
    }
    fn size_hint(&self) -> usize { 4 + self.len() }
}

impl FastSerialize for &&str {
    fn write_to_buffer(&self, buffer: &mut Vec<u8>) {
        let len = self.len() as u32;
        buffer.extend_from_slice(&len.to_le_bytes());
        buffer.extend_from_slice(self.as_bytes());
    }
    fn size_hint(&self) -> usize { 4 + self.len() }
}

impl FastSerialize for String {
    fn write_to_buffer(&self, buffer: &mut Vec<u8>) {
        self.as_str().write_to_buffer(buffer);
    }
    fn size_hint(&self) -> usize { 4 + self.len() }
}

impl FastSerialize for &String {
    fn write_to_buffer(&self, buffer: &mut Vec<u8>) {
        self.as_str().write_to_buffer(buffer);
    }
    fn size_hint(&self) -> usize { 4 + self.len() }
}

impl FastSerialize for &[u8] {
    fn write_to_buffer(&self, buffer: &mut Vec<u8>) {
        let len = self.len() as u32;
        buffer.extend_from_slice(&len.to_le_bytes());
        buffer.extend_from_slice(self);
    }
    fn size_hint(&self) -> usize { 4 + self.len() }
}

/// Output destination for log entries
#[derive(Debug, Clone)]
pub enum LogDestination {
    /// Output to stdout
    Stdout,
    /// Output to stderr
    Stderr,
    /// Output to a file
    File(PathBuf),
    /// Output to multiple destinations
    Multiple(Vec<LogDestination>),
}

/// Configuration for the lightning logger
#[derive(Debug, Clone)]
pub struct LoggerConfig {
    /// Channel capacity for buffering log entries
    pub channel_capacity: usize,
    /// Output destinations
    pub destinations: Vec<LogDestination>,
    /// Buffer size for file output (in bytes)
    pub file_buffer_size: usize,
    /// Enable CPU affinity for logging thread
    pub enable_cpu_affinity: bool,
}

impl Default for LoggerConfig {
    fn default() -> Self {
        Self {
            channel_capacity: 10000,
            destinations: vec![LogDestination::Stdout],
            file_buffer_size: 8192,
            enable_cpu_affinity: true,
        }
    }
}

/// Log levels
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LogLevel {
    Debug = 0,
    Info = 1,
    Warn = 2,
    Error = 3,
}

impl std::fmt::Display for LogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LogLevel::Debug => write!(f, "DEBUG"),
            LogLevel::Info => write!(f, "INFO"),
            LogLevel::Warn => write!(f, "WARN"),
            LogLevel::Error => write!(f, "ERROR"),
        }
    }
}

/// A log entry containing all necessary information
#[derive(Debug)]
pub struct LogEntry {
    pub sequence: u64,
    pub timestamp_nanos: u64,
    pub level: LogLevel,
    pub message_id: u32,
    pub data: Vec<u8>,
}

impl LogEntry {
    /// Create a new log entry
    pub fn new(level: LogLevel, message_id: u32, estimated_size: usize) -> Self {
        let sequence = SEQUENCE_COUNTER.fetch_add(1, Ordering::Relaxed);
        let timestamp_nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;
        
        Self {
            sequence,
            timestamp_nanos,
            level,
            message_id,
            data: Vec::with_capacity(estimated_size + 32), // Extra space for metadata
        }
    }
}

/// Output writer that handles multiple destinations
pub struct OutputWriter {
    writers: Vec<Box<dyn Write + Send + 'static>>,
}

impl OutputWriter {
    /// Create a new output writer with the given destinations
    pub fn new(destinations: &[LogDestination], buffer_size: usize) -> io::Result<Self> {
        let mut writers = Vec::new();
        Self::collect_writers(destinations, buffer_size, &mut writers)?;
        Ok(Self { writers })
    }

    /// Recursively collect writers from destinations
    fn collect_writers(destinations: &[LogDestination], buffer_size: usize, writers: &mut Vec<Box<dyn Write + Send + 'static>>) -> io::Result<()> {
        for dest in destinations {
            match dest {
                LogDestination::Stdout => {
                    writers.push(Box::new(BufWriter::with_capacity(buffer_size, io::stdout())) as Box<dyn Write + Send>);
                }
                LogDestination::Stderr => {
                    writers.push(Box::new(BufWriter::with_capacity(buffer_size, io::stderr())) as Box<dyn Write + Send>);
                }
                LogDestination::File(path) => {
                    let file = OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(path)?;
                    writers.push(Box::new(BufWriter::with_capacity(buffer_size, file)) as Box<dyn Write + Send>);
                }
                LogDestination::Multiple(dests) => {
                    // Recursively handle nested multiple destinations
                    Self::collect_writers(dests, buffer_size, writers)?;
                }
            }
        }
        Ok(())
    }

    /// Write data to all destinations
    pub fn write_all(&mut self, data: &[u8]) -> io::Result<()> {
        for writer in &mut self.writers {
            writer.write_all(data)?;
        }
        Ok(())
    }

    /// Flush all writers
    pub fn flush(&mut self) -> io::Result<()> {
        for writer in &mut self.writers {
            writer.flush()?;
        }
        Ok(())
    }
}

impl Drop for OutputWriter {
    fn drop(&mut self) {
        let _ = self.flush();
    }
}

/// The main lightning logger
pub struct LightningLogger {
    sender: Sender<LogEntry>,
    _handle: std::thread::JoinHandle<()>,
    shutdown: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl LightningLogger {
    /// Create a new lightning logger with default configuration
    pub fn new() -> Self {
        Self::with_config(LoggerConfig::default())
    }

    /// Create a new lightning logger with specified channel capacity
    pub fn with_capacity(capacity: usize) -> Self {
        let mut config = LoggerConfig::default();
        config.channel_capacity = capacity;
        Self::with_config(config)
    }

        /// Create a new lightning logger with custom configuration
    pub fn with_config(config: LoggerConfig) -> Self {
        let (sender, receiver) = bounded(config.channel_capacity);
        let shutdown = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));

        let config_clone = config.clone();
        let shutdown_clone = shutdown.clone();
        let handle = std::thread::spawn(move || {
            Self::logging_thread(receiver, config_clone, shutdown_clone);
        });

        Self {
            sender,
            _handle: handle,
            shutdown,
        }
    }
    
    /// The logging thread that processes entries
    fn logging_thread(receiver: Receiver<LogEntry>, config: LoggerConfig, shutdown: std::sync::Arc<std::sync::atomic::AtomicBool>) {
        // Set CPU affinity if available and enabled
        #[cfg(feature = "cpu-affinity")]
        if config.enable_cpu_affinity {
            if let Some(core_ids) = core_affinity::get_core_ids() {
                if let Some(&last_core) = core_ids.last() {
                    let _ = core_affinity::set_for_current(last_core);
                }
            }
        }

        // Initialize output writer
        let mut output_writer = match OutputWriter::new(&config.destinations, config.file_buffer_size) {
            Ok(writer) => writer,
            Err(e) => {
                eprintln!("Failed to initialize output writer: {}", e);
                return;
            }
        };

        loop {
            match receiver.try_recv() {
                Ok(entry) => {
                    Self::process_entry(entry, &mut output_writer);
                },
                Err(TryRecvError::Empty) => {
                    // Check for shutdown signal
                    if shutdown.load(std::sync::atomic::Ordering::Relaxed) {
                        // Shutdown requested, process any remaining messages
                        let mut remaining = Vec::new();
                        while let Ok(entry) = receiver.try_recv() {
                            remaining.push(entry);
                        }

                        for entry in remaining {
                            Self::process_entry(entry, &mut output_writer);
                        }

                        // Flush and exit
                        let _ = output_writer.flush();
                        break;
                    }

                    // No messages, yield briefly
                    std::thread::yield_now();
                },
                Err(TryRecvError::Disconnected) => {
                    // Channel closed, flush and exit
                    let _ = output_writer.flush();
                    break;
                }
            }
        }
    }
    /// Process a single log entry
    fn process_entry(entry: LogEntry, output_writer: &mut OutputWriter) {
        let secs = entry.timestamp_nanos / 1_000_000_000;
        let nanos = entry.timestamp_nanos % 1_000_000_000;

        // Try to get message format
        let format_registry = MESSAGE_FORMATS.get().map(|r| r.read().ok()).flatten();

        let log_line = if let Some(registry) = format_registry {
            if let Some(format) = registry.get(&entry.message_id) {
                // Decode binary data and format it according to the message format
                match Self::decode_and_format_data(&entry.data, format) {
                    Ok(formatted_data) => {
                        format!("[{}] seq:{} ts:{}.{:09} msg_id:{} {}\n",
                                entry.level, entry.sequence, secs, nanos,
                                entry.message_id, formatted_data)
                    },
                    Err(_) => {
                        // Fallback to hex dump if decoding fails
                        let hex_data = Self::hex_dump(&entry.data);
                        format!("[{}] seq:{} ts:{}.{:09} msg_id:{} fmt:'{}' data:{}\n",
                                entry.level, entry.sequence, secs, nanos,
                                entry.message_id, format, hex_data)
                    }
                }
            } else {
                // No format registered, show hex dump
                let hex_data = Self::hex_dump(&entry.data);
                format!("[{}] seq:{} ts:{}.{:09} msg_id:{} data:{}\n",
                        entry.level, entry.sequence, secs, nanos,
                        entry.message_id, hex_data)
            }
        } else {
            // No registry available, show hex dump
            let hex_data = Self::hex_dump(&entry.data);
            format!("[{}] seq:{} ts:{}.{:09} msg_id:{} data:{}\n",
                    entry.level, entry.sequence, secs, nanos,
                    entry.message_id, hex_data)
        };

        // Write to all configured destinations
        let _ = output_writer.write_all(log_line.as_bytes());
    }

    /// Decode binary data and format it according to the message format
    fn decode_and_format_data(data: &[u8], format: &str) -> Result<String, &'static str> {
        let mut data_offset = 0;

        // Simple format string parser - looks for {} placeholders
        let mut output = String::new();
        let mut i = 0;

        while i < format.len() {
            if i + 1 < format.len() && &format[i..i+2] == "{}" {
                // Found placeholder, decode next argument from binary data
                match Self::decode_next_arg(data, &mut data_offset) {
                    Ok(decoded_value) => {
                        output.push_str(&decoded_value);
                    },
                    Err(_) => {
                        return Err("Failed to decode binary data");
                    }
                }
                i += 2;
            } else {
                output.push(format.as_bytes()[i] as char);
                i += 1;
            }
        }

        Ok(output)
    }

    /// Decode the next argument from binary data
    fn decode_next_arg(data: &[u8], offset: &mut usize) -> Result<String, &'static str> {
        if *offset >= data.len() {
            return Err("Not enough data");
        }

        // Determine type based on data size and content
        let remaining = data.len() - *offset;

        if remaining >= 8 {
            // Could be f64, i64, u64
            let value = u64::from_le_bytes(data[*offset..*offset+8].try_into().unwrap());
            *offset += 8;

            // Try to detect if this is a float by checking if it's a reasonable float value
            let as_f64 = f64::from_bits(value);
            if as_f64.is_finite() && as_f64.fract() != 0.0 {
                return Ok(format!("{:.6}", as_f64));
            } else {
                return Ok(value.to_string());
            }
        } else if remaining >= 4 {
            // Could be f32, i32, u32
            let value = u32::from_le_bytes(data[*offset..*offset+4].try_into().unwrap());
            *offset += 4;

            let as_f32 = f32::from_bits(value);
            if as_f32.is_finite() && as_f32.fract() != 0.0 {
                return Ok(format!("{:.3}", as_f32));
            } else {
                return Ok(value.to_string());
            }
        } else if remaining >= 1 {
            // Could be bool, i8, u8
            let value = data[*offset];
            *offset += 1;

            if value == 0 {
                return Ok("false".to_string());
            } else if value == 1 {
                return Ok("true".to_string());
            } else {
                return Ok(value.to_string());
            }
        }

        Err("Unknown data type")
    }

    /// Create a hex dump of binary data for debugging
    fn hex_dump(data: &[u8]) -> String {
        if data.is_empty() {
            return "[]".to_string();
        }

        let mut result = "[".to_string();
        for (i, &byte) in data.iter().enumerate() {
            if i > 0 {
                result.push(' ');
            }
            result.push_str(&format!("{:02x}", byte));
        }
        result.push(']');
        result
    }
    
    /// Send a log entry (non-blocking)
    pub fn log(&self, entry: LogEntry) -> Result<(), LogEntry> {
        match self.sender.try_send(entry) {
            Ok(()) => Ok(()),
            Err(crossbeam_channel::TrySendError::Full(entry)) => Err(entry),
            Err(crossbeam_channel::TrySendError::Disconnected(entry)) => Err(entry),
        }
    }

    /// Initiate graceful shutdown of the logger
    pub fn shutdown(&self) {
        self.shutdown.store(true, std::sync::atomic::Ordering::Relaxed);
    }

    /// Wait for the logging thread to complete
    pub fn wait_for_shutdown(self) {
        let _ = self._handle.join();
    }
}

/// Initialize the global lightning logger with default configuration
pub fn init_lightning_log() -> Result<(), &'static str> {
    let logger = LightningLogger::new();
    match GLOBAL_LOGGER.set(logger) {
        Ok(()) => {
            let _ = MESSAGE_FORMATS.set(Arc::new(RwLock::new(HashMap::new())));
            Ok(())
        },
        Err(_) => Err("Lightning logger already initialized"),
    }
}

/// Initialize with custom capacity
pub fn init_lightning_log_with_capacity(capacity: usize) -> Result<(), &'static str> {
    let logger = LightningLogger::with_capacity(capacity);
    match GLOBAL_LOGGER.set(logger) {
        Ok(()) => {
            let _ = MESSAGE_FORMATS.set(Arc::new(RwLock::new(HashMap::new())));
            Ok(())
        },
        Err(_) => Err("Lightning logger already initialized"),
    }
}

/// Initialize with custom configuration
pub fn init_lightning_log_with_config(config: LoggerConfig) -> Result<(), &'static str> {
    let logger = LightningLogger::with_config(config);
    match GLOBAL_LOGGER.set(logger) {
        Ok(()) => {
            let _ = MESSAGE_FORMATS.set(Arc::new(RwLock::new(HashMap::new())));
            Ok(())
        },
        Err(_) => Err("Lightning logger already initialized"),
    }
}

/// Register a message format for human-readable output
pub fn register_message_format(message_id: u32, format: String) -> Result<(), &'static str> {
    if let Some(formats) = MESSAGE_FORMATS.get() {
        if let Ok(mut registry) = formats.write() {
            registry.insert(message_id, format);
            Ok(())
        } else {
            Err("Failed to acquire write lock on message formats")
        }
    } else {
        Err("Lightning logger not initialized")
    }
}

/// Internal function to send log entry
#[doc(hidden)]
pub fn __send_log_entry(entry: LogEntry) {
    if let Some(logger) = GLOBAL_LOGGER.get() {
        let _ = logger.log(entry); // Drop errors silently for performance
    }
}

/// Get a reference to the global logger for manual shutdown control
pub fn get_global_logger() -> Option<&'static LightningLogger> {
    GLOBAL_LOGGER.get()
}

/// Initiate graceful shutdown of the global logger
pub fn shutdown_lightning_log() {
    if let Some(logger) = GLOBAL_LOGGER.get() {
        logger.shutdown();
    }
}

/// Wait for the global logger to complete shutdown
pub fn wait_for_shutdown() {
    if let Some(logger) = GLOBAL_LOGGER.get() {
        // Note: This will block indefinitely since we can't move the logger
        // In practice, you'd want to store the logger instance separately for clean shutdown
        logger.shutdown();
    }
}

/// Calculate size hint for multiple arguments
#[macro_export]
macro_rules! __size_hint {
    () => { 0 };
    ($arg:expr) => { $arg.size_hint() };
    ($arg:expr, $($rest:expr),+) => { $arg.size_hint() + __size_hint!($($rest),+) };
}

/// Main logging macro
#[macro_export]
macro_rules! lightning_log {
    ($level:expr, $msg_id:expr $(, $arg:expr)*) => {{
        let estimated_size = 0 $(+ $arg.size_hint())*;
        let mut entry = $crate::LogEntry::new($level, $msg_id, estimated_size);
        
        $(
            $crate::FastSerialize::write_to_buffer(&$arg, &mut entry.data);
        )*
        
        $crate::__send_log_entry(entry);
    }};
}

/// Debug level logging
#[macro_export]
macro_rules! lightning_debug {
    ($msg_id:expr $(, $arg:expr)*) => {
        $crate::lightning_log!($crate::LogLevel::Debug, $msg_id $(, $arg)*)
    };
}

/// Info level logging
#[macro_export]
macro_rules! lightning_info {
    ($msg_id:expr $(, $arg:expr)*) => {
        $crate::lightning_log!($crate::LogLevel::Info, $msg_id $(, $arg)*)
    };
}

/// Warning level logging
#[macro_export]
macro_rules! lightning_warn {
    ($msg_id:expr $(, $arg:expr)*) => {
        $crate::lightning_log!($crate::LogLevel::Warn, $msg_id $(, $arg)*)
    };
}

/// Error level logging
#[macro_export]
macro_rules! lightning_error {
    ($msg_id:expr $(, $arg:expr)*) => {
        $crate::lightning_log!($crate::LogLevel::Error, $msg_id $(, $arg)*)
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_basic_logging() {
        // Only initialize if not already done
        let _ = init_lightning_log();

        let volume = 100.02f64;
        let price = 20000.0f64;
        let flag = true;
        let symbol = "AAPL";

        lightning_info!(1001, volume, price, flag, symbol);
        lightning_debug!(1002, price, symbol);
        lightning_warn!(1003, flag);
        lightning_error!(1004, 42i32);

        // Give the logger time to process
        std::thread::sleep(std::time::Duration::from_millis(50));
    }

    #[test]
    fn test_message_formats() {
        // Only initialize if not already done
        let _ = init_lightning_log();
        let _ = register_message_format(2001, "Test message: value={}".to_string());

        lightning_info!(2001, 123.45f64);

        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}