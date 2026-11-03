//! `tracing` subscriber setup for console and journald output.

use std::{fmt, io, time::SystemTime};

use tracing_subscriber::{
    fmt::{
        format::{FmtSpan, Writer},
        time::FormatTime,
    },
    prelude::*,
    EnvFilter, Layer, Registry,
};

use crate::config::logger::{ConsoleConfig, LogFormat, LoggerConfig, TimestampFormat};

#[derive(Debug, thiserror::Error)]
pub enum LoggerError {
    #[error("invalid log level '{level}': {source}")]
    Level {
        level: String,
        #[source]
        source: tracing_subscriber::filter::ParseError,
    },
    #[error("journald is the only configured output and it is unavailable: {0}")]
    Journald(#[source] io::Error),
    #[error("no log output is enabled")]
    NoOutput,
    #[error("logger already initialized")]
    AlreadySet,
}

/// Installs the global subscriber. `RUST_LOG` takes precedence over
/// `config.level` so operators can raise verbosity without editing the file.
pub fn init(config: &LoggerConfig) -> Result<(), LoggerError> {
    let filter = match std::env::var("RUST_LOG") {
        Ok(directives) if !directives.is_empty() => {
            EnvFilter::try_new(&directives).map_err(|source| LoggerError::Level {
                level: directives,
                source,
            })?
        }
        _ => EnvFilter::try_new(&config.level).map_err(|source| LoggerError::Level {
            level: config.level.clone(),
            source,
        })?,
    };

    let console = config
        .console
        .enabled
        .then(|| console_layer(&config.console, config.timestamp_format));

    let journald = if config.journald.enabled {
        match tracing_journald::layer() {
            Ok(layer) => Some(layer.with_syslog_identifier(config.journald.identifier.clone())),
            Err(err) if console.is_some() => {
                eprintln!("blazebee: journald unavailable ({err}); logging to console only");
                None
            }
            Err(err) => return Err(LoggerError::Journald(err)),
        }
    } else {
        None
    };

    if console.is_none() && journald.is_none() {
        return Err(LoggerError::NoOutput);
    }

    Registry::default()
        .with(console)
        .with(journald)
        .with(filter)
        .try_init()
        .map_err(|_| LoggerError::AlreadySet)
}

type BoxedLayer = Box<dyn Layer<Registry> + Send + Sync>;

fn console_layer(config: &ConsoleConfig, timestamps: TimestampFormat) -> BoxedLayer {
    let span_events = if config.show_spans {
        FmtSpan::CLOSE
    } else {
        FmtSpan::NONE
    };
    let base = tracing_subscriber::fmt::layer()
        .with_target(config.show_target)
        .with_thread_ids(config.show_thread_ids)
        .with_span_events(span_events)
        .with_ansi(config.ansi_colors)
        .with_timer(Timer(timestamps))
        .with_writer(io::stdout);
    match config.format {
        LogFormat::Compact => base.compact().boxed(),
        LogFormat::Pretty => base.pretty().boxed(),
        LogFormat::Json => base.json().boxed(),
    }
}

struct Timer(TimestampFormat);

impl FormatTime for Timer {
    fn format_time(&self, w: &mut Writer<'_>) -> fmt::Result {
        match self.0 {
            TimestampFormat::Rfc3339 => tracing_subscriber::fmt::time::SystemTime.format_time(w),
            TimestampFormat::Unix => {
                let now = SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap_or_default();
                write!(w, "{}.{:06}", now.as_secs(), now.subsec_micros())
            }
        }
    }
}
