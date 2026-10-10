//! Database initialization.
//!
//! - [`migrating`] — builds `sqlx::migrate::Migration`s the way
//!   `sqlx::migrate!` does, so `_sqlx_migrations` checksums match.
//! - [`seeding`] — [`seeding::Seeder`] runs [`seeding::Seed`]s.
//!
//! ```rust,ignore
//! let pool = sqlx::PgPool::connect(&database_url).await?;
//!
//! pgkit::migrator!("src").run(&pool).await?; // an sqlx::migrate::Migrator
//! pgkit::seeder!("src").run(&pool).await?;   // a seeding::Seeder
//! ```
//!
//! Collections compose across crates: a library exports the
//! `Vec<sqlx::migrate::Migration>` that `pgkit::migrations!` expands to
//! (the SQL is embedded into the library's own build), and the application
//! merges the vecs, sorts by version, and runs them as one
//! [`migrating::migrator`]. See "Merging collections from multiple crates"
//! in the README.

pub mod migrating;
pub mod seeding;
