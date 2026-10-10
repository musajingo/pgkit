use sqlx::migrate::{Migration, Migrator};

/// Builds a SQLx migrator, sorting migrations by version.
///
/// Does not reject duplicate versions: check for collisions when merging
/// collections. `pgkit::migrator!` calls this after checking its own scan.
pub fn migrator(migrations: Vec<Migration>) -> Migrator {
    Migrator::with_migrations(migrations)
}
