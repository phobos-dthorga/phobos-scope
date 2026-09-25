//! Game-independent, offline analysis. All operation timings are inclusive elapsed time.
mod analysis;
mod comparison;
mod export;
mod html;
mod model;
mod validation;

pub use analysis::*;
pub use comparison::*;
pub use export::*;
pub use html::*;
pub use model::*;
pub use validation::*;

use std::{fmt, io::Read};

#[derive(Debug)]
pub struct Error {
    pub code: &'static str,
    pub detail: String,
}

impl Error {
    pub(crate) fn new(code: &'static str, detail: impl Into<String>) -> Self {
        Self {
            code,
            detail: detail.into(),
        }
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.detail)
    }
}
impl std::error::Error for Error {}

pub fn read_capture(reader: impl Read) -> Result<ValidatedCapture, Error> {
    let mut bytes = Vec::new();
    reader
        .take(MAX_INPUT_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| Error::new("capture.read", e.to_string()))?;
    if bytes.len() as u64 > MAX_INPUT_BYTES {
        return Err(Error::new(
            "capture.size",
            "Capture exceeds the 64 MiB input limit.",
        ));
    }
    // Inspect the version before deserializing a potentially different schema.
    #[derive(serde::Deserialize)]
    struct Version {
        format_version: Option<u32>,
    }
    let version: Version = serde_json::from_slice(&bytes)
        .map_err(|e| Error::new("capture.json", format!("Invalid or truncated JSON: {e}")))?;
    if version.format_version != Some(FORMAT_VERSION) {
        return Err(Error::new(
            "capture.version",
            "Expected format_version 1; use a compatible analyser.",
        ));
    }
    // Deserialize the original bytes so duplicate object keys remain an error.
    let capture: Capture =
        serde_json::from_slice(&bytes).map_err(|e| Error::new("capture.schema", e.to_string()))?;
    validate(capture)
}
