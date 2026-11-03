use std::{io, time::Duration};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum CollectorError {
    #[error("cannot read {path}: {source}")]
    Read {
        path: String,
        #[source]
        source: io::Error,
    },

    #[error("cannot parse {what} in {path}: {reason}")]
    Parse {
        path: String,
        what: &'static str,
        reason: String,
    },

    #[error("{path}: missing {what}")]
    Missing { path: String, what: &'static str },

    #[error("command `{command}` failed: {reason}")]
    Command {
        command: &'static str,
        reason: String,
    },

    #[error("collection did not finish within {0:?}")]
    Timeout(Duration),

    #[error("{0}")]
    Unsupported(&'static str),
}

impl CollectorError {
    pub fn read(path: impl Into<String>, source: io::Error) -> Self {
        CollectorError::Read {
            path: path.into(),
            source,
        }
    }

    pub fn parse(path: impl Into<String>, what: &'static str, reason: impl Into<String>) -> Self {
        CollectorError::Parse {
            path: path.into(),
            what,
            reason: reason.into(),
        }
    }

    pub fn missing(path: impl Into<String>, what: &'static str) -> Self {
        CollectorError::Missing {
            path: path.into(),
            what,
        }
    }
}
