use std::borrow::Cow;

use sqlx::SqlStr;
use sqlx::migrate::{Migration, MigrationType};

/// Builds an up [`Migration`] with SQLx's SQL checksum.
/// Does not validate the SQL or check version uniqueness.
pub fn migration(
    version: i64,
    description: &'static str,
    sql: &'static str,
    no_tx: bool,
) -> Migration {
    Migration::new(
        version,
        Cow::Borrowed(description),
        MigrationType::ReversibleUp,
        SqlStr::from_static(sql),
        no_tx,
    )
}
