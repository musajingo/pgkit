/// A single seed script.
#[derive(Debug, Clone, Copy)]
pub struct Seed {
    /// Label for diagnostics; the macro uses the path relative to its scan root.
    pub path: &'static str,
    /// SQL to execute. No file is read at runtime.
    pub sql: &'static str,
}
