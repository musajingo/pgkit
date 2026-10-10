//! Database initialization.
//!
//! - [`migrating`] builds SQLx migrations and migrators.
//! - [`seeding`] runs seed SQL in slice order on every call.
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
//! merges the vectors, checks for duplicate versions, and passes them to
//! [`migrating::migrator`], which sorts by version. Each migration keeps its
//! own history entry and transaction unless the file starts with `-- no-transaction`.
//! The macros check file names, not SQL syntax. See the README's
//! "Merging collections from multiple crates" section for examples.

pub mod migrating;
pub mod seeding;
