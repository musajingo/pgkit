/// Retry classification of the failed statement or connection attempt.
/// Does not account for earlier successful work in a multi-step operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetrySafety {
    /// The outcome won't change on retry (constraint violations, missing
    /// rows, syntax errors, …).
    Never,
    /// The statement never executed or definitively rolled back (pool/connect
    /// rejections, deadlocks, serialization failures). Earlier successful
    /// statements or external side effects may still need protection.
    Always,
    /// The failure happened mid-statement (I/O, TLS, connection dropped), so
    /// the database may have already applied the work. Retrying is only safe
    /// when the operation is idempotent.
    IfIdempotent,
}
