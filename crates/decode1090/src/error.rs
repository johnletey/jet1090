use serde::{Deserialize, Serialize};
use std::fmt;

/// Error codes for structured error reporting
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    InvalidHexPayload,
    InvalidPayloadLength,
    InvalidBdsCode,
    DecodingFailed,
    FileNotFound,
    InvalidFilePath,
    ParsingError,
    IoError,
    SerializationError,
    InvalidArgument,
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ErrorCode::InvalidHexPayload => write!(f, "INVALID_HEX_PAYLOAD"),
            ErrorCode::InvalidPayloadLength => {
                write!(f, "INVALID_PAYLOAD_LENGTH")
            }
            ErrorCode::InvalidBdsCode => write!(f, "INVALID_BDS_CODE"),
            ErrorCode::DecodingFailed => write!(f, "DECODING_FAILED"),
            ErrorCode::FileNotFound => write!(f, "FILE_NOT_FOUND"),
            ErrorCode::InvalidFilePath => write!(f, "INVALID_FILE_PATH"),
            ErrorCode::ParsingError => write!(f, "PARSING_ERROR"),
            ErrorCode::IoError => write!(f, "IO_ERROR"),
            ErrorCode::SerializationError => write!(f, "SERIALIZATION_ERROR"),
            ErrorCode::InvalidArgument => write!(f, "INVALID_ARGUMENT"),
        }
    }
}

/// Structured error response for JSON output
#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub error: String,
    pub code: ErrorCode,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<serde_json::Value>,
}

impl ErrorResponse {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            error: message.into(),
            code,
            context: None,
        }
    }

    pub fn with_context(
        code: ErrorCode,
        message: impl Into<String>,
        context: serde_json::Value,
    ) -> Self {
        Self {
            error: message.into(),
            code,
            context: Some(context),
        }
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

}

impl fmt::Display for ErrorResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.error)
    }
}

impl std::error::Error for ErrorResponse {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_response_serialization() {
        let err = ErrorResponse::new(
            ErrorCode::InvalidHexPayload,
            "Hex string contains invalid characters",
        );
        let json = err.to_json().unwrap();
        assert!(json.contains("INVALID_HEX_PAYLOAD"));
        assert!(json.contains("Hex string contains invalid characters"));
    }

    #[test]
    fn test_error_response_with_context() {
        let context = serde_json::json!({
            "payload_length": 8,
            "expected_length": 7
        });
        let err = ErrorResponse::with_context(
            ErrorCode::InvalidPayloadLength,
            "Payload must be 7 bytes",
            context,
        );
        let json = err.to_json().unwrap();
        assert!(json.contains("INVALID_PAYLOAD_LENGTH"));
        assert!(json.contains("payload_length"));
    }
}
