use std::time::Duration;

/// Connection limits and session timeouts for [`get`](super::get).
///
/// Construct it with struct-update syntax so unset fields keep their
/// defaults:
///
/// ```
/// use std::time::Duration;
/// use pgkit::pool::PoolConfig;
///
/// let config = PoolConfig {
///     max_connections: 20,
///     ..PoolConfig::default()
/// };
/// assert_eq!(config.statement_timeout, Duration::from_secs(30));
/// ```
///
/// The two session timeouts follow Postgres's own convention, a value of
/// `Duration::ZERO` disables that timeout. For migrations that need longer
/// than the defaults, adjust or disable them:
///
/// ```
/// use std::time::Duration;
/// use pgkit::pool::PoolConfig;
///
/// let config = PoolConfig {
///     statement_timeout: Duration::ZERO,
///     idle_tx_timeout: Duration::ZERO,
///     ..PoolConfig::default()
/// };
/// ```
#[derive(Debug)]
pub struct PoolConfig {
    /// Upper bound on open connections. Default `5`.
    ///
    /// [`get`](super::get) also sets a minimum target for open connections:
    /// a quarter of this value, clamped to 1–5 and capped by `max_connections`.
    /// The target counts both idle and checked-out connections; SQLx
    /// maintains it on a best-effort basis.
    pub max_connections: u32,
    /// Cancels statements exceeding this timeout (Postgres
    /// `statement_timeout`, set per session). `ZERO` disables it.
    /// Default 30 seconds.
    pub statement_timeout: Duration,
    /// Terminates sessions idle inside an open transaction longer than
    /// this (Postgres `idle_in_transaction_session_timeout`).
    /// `ZERO` disables it. Default 60 seconds.
    pub idle_tx_timeout: Duration,
    /// How long a caller waits for a connection from the pool before
    /// erroring. Default 3 seconds.
    pub acquire_timeout: Duration,
    /// Closes connections that have been idle for this long.
    /// Default 10 minutes.
    pub idle_timeout: Duration,
    /// Closes any connection older than this. Default 30 minutes.
    pub max_lifetime: Duration,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            max_connections: 5,
            statement_timeout: Duration::from_secs(30),
            idle_tx_timeout: Duration::from_secs(60),
            acquire_timeout: Duration::from_secs(3),
            idle_timeout: Duration::from_secs(600),
            max_lifetime: Duration::from_secs(1800),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // The literals pin the defaults each field documents; changing a
    // default must update its field doc.
    #[test]
    fn defaults_match_field_docs() {
        let d = PoolConfig::default();
        assert_eq!(d.max_connections, 5);
        assert_eq!(d.statement_timeout, Duration::from_secs(30));
        assert_eq!(d.idle_tx_timeout, Duration::from_secs(60));
        assert_eq!(d.acquire_timeout, Duration::from_secs(3));
        assert_eq!(d.idle_timeout, Duration::from_secs(600));
        assert_eq!(d.max_lifetime, Duration::from_secs(1800));
    }
}
