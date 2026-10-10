use super::cursor_error::CursorError;

/// Errors from cursor pagination, projection checks, and cursor encoding.
#[derive(Debug, thiserror::Error)]
pub enum CursorPaginationError {
    /// The column used for identifying the row was not in the selected columns.
    #[error("id column not projected")]
    IdColumnWasNotProjected,

    /// The column used for ordering is not in the selected columns.
    #[error("ordering column \"{column}\" was not in the selected columns")]
    OrderingColumnNotProjected { column: String },

    /// Errors that might happen when encoding/decoding a cursor
    #[error("error encoding or decoding the cursor: {0}")]
    CursorError(#[from] CursorError),

    /// A server-side pagination failure, such as serializing an ordering value.
    #[error("something went wrong in cursor pagination: {message}")]
    InternalError {
        message: String,
        source: Box<dyn std::error::Error + Send + Sync>,
    },
}
