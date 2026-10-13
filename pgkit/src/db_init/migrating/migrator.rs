use sqlx::migrate::{Migration, Migrator};

/// Builds a SQLx migrator, sorting migrations by version.
///
/// Does not reject duplicate versions: use [`super::merge_migrations`] when
/// combining collections. `pgkit::migrator!` calls this after checking its own scan.
pub fn migrator(migrations: Vec<Migration>) -> Migrator {
    Migrator::with_migrations(migrations)
}
