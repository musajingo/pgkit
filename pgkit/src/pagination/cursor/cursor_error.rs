/// Errors that can occur during cursor encode/decode.
#[derive(Debug, thiserror::Error)]
pub enum CursorError {
    /// Base64 decoding failed.
    #[error("Base64 error: {0}")]
    Base64(#[from] base64::DecodeError),

    /// UTF-8 decoding failed.
    #[error("UTF-8 error: {0}")]
    Utf8(#[from] std::string::FromUtf8Error),

    /// JSON serialization or deserialization failed.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// The cursor value and sort column belong to incompatible type classes.
    #[error("cursor value {value} does not fit sort column of type {column}")]
    ValueTypeMismatch { value: &'static str, column: String },
}
