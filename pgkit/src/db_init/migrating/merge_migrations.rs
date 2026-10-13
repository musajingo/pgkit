use std::fmt;

use sqlx::migrate::{Migration, Migrator};

use super::migrator::migrator;

/// Two merged collections claimed the same migration version.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DuplicateMigrationVersion {
    /// The version number present in more than one collection.
    pub version: i64,
}

impl fmt::Display for DuplicateMigrationVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "duplicate migration version {}", self.version)
    }
}

impl std::error::Error for DuplicateMigrationVersion {}

/// Merges migration collections into one [`Migrator`], sorting by version and
/// rejecting version collisions before any SQL runs.
///
/// A library exports the `Vec<sqlx::migrate::Migration>` that
/// `pgkit::migrations!` expands to; the application merges that vector with
/// its own collection here. Each migration keeps its own history entry.
///
/// ```rust,ignore
/// let migrator = pgkit::db_init::migrating::merge_migrations([
///     pgkit::migrations!("src"),
///     some_library::migrations(),
/// ])?;
/// migrator.run(&pool).await?;
/// ```
pub fn merge_migrations(
    collections: impl IntoIterator<Item = Vec<Migration>>,
) -> Result<Migrator, DuplicateMigrationVersion> {
    let mut all: Vec<_> = collections.into_iter().flatten().collect();
    all.sort_by_key(|migration| migration.version);
    if let Some(pair) = all
        .windows(2)
        .find(|pair| pair[0].version == pair[1].version)
    {
        return Err(DuplicateMigrationVersion {
            version: pair[0].version,
        });
    }
    Ok(migrator(all))
}

#[cfg(test)]
mod tests {
    use super::super::migration::migration;
    use super::{DuplicateMigrationVersion, merge_migrations};

    #[test]
    fn merges_and_sorts_across_collections() {
        let merged = merge_migrations([
            vec![migration(3, "c", "SELECT 3;", false)],
            vec![
                migration(1, "a", "SELECT 1;", false),
                migration(2, "b", "SELECT 2;", false),
            ],
        ])
        .unwrap();
        let versions: Vec<_> = merged.iter().map(|migration| migration.version).collect();
        assert_eq!(versions, [1, 2, 3]);
    }

    #[test]
    fn empty_input_builds_an_empty_migrator() {
        let merged = merge_migrations([]).unwrap();
        assert_eq!(merged.iter().count(), 0);
    }

    #[test]
    fn a_version_collision_is_rejected_and_named() {
        let error = merge_migrations([
            vec![migration(7, "ours", "SELECT 1;", false)],
            vec![migration(7, "theirs", "SELECT 2;", false)],
        ])
        .unwrap_err();
        assert_eq!(error, DuplicateMigrationVersion { version: 7 });
        assert_eq!(error.to_string(), "duplicate migration version 7");
    }

    #[test]
    fn distinct_versions_in_one_collection_pass() {
        let merged = merge_migrations([vec![
            migration(1, "a", "SELECT 1;", false),
            migration(2, "b", "SELECT 2;", false),
        ]])
        .unwrap();
        assert_eq!(merged.iter().count(), 2);
    }
}
