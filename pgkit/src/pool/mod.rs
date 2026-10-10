//! Opens a [`sqlx::PgPool`] with configurable limits and timeouts; see
//! [`PoolConfig`] for each knob and its default.

mod config;
mod get;

pub use config::PoolConfig;
pub use get::get;
