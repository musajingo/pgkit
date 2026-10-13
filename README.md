# pgkit

[![Crates.io](https://img.shields.io/crates/v/pgkit.svg)](https://crates.io/crates/pgkit)
[![Documentation](https://docs.rs/pgkit/badge.svg)](https://docs.rs/pgkit)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)

Postgres utilities for Rust, built on [sqlx](https://github.com/launchbadge/sqlx).

pgkit provides query builders, row-derived table and column types, pagination,
retries, database errors, and embedded migrations and seeds. SQLx executes the
queries. PostgreSQL checks SQL syntax and whether tables and columns exist.

> **Status:** pre-1.0. The API may change between minor versions.

## Features

- **Query builder** for `SELECT`, `INSERT`, `UPDATE` and `DELETE`, with joins,
  CTEs, `UNION`, `ON CONFLICT`, `RETURNING` and row locking. Value methods bind
  parameters; raw SQL methods insert the SQL you supply.
- **`#[derive(PgKit)]`** generates a table type, a column enum and a partial
  row from one struct. Rust checks references to those generated types.
- **Pagination** using `LIMIT` / `OFFSET` or a keyset cursor.
- **`#[pgkit::retry]`** repeats an async function on selected transient errors.
- **`RepositoryError`** classifies constraint violations, missing rows and
  other database errors, preserving constraint names when available.
- **Migrations and seeds** embedded from SQL files at compile time.
- **Full-text search helper** for prefix-matching `to_tsquery` input.
- **Connection pool helper** with configurable limits and session timeouts.

## Installation

```toml
[dependencies]
pgkit = "0.4"
sqlx = { version = "0.9", default-features = false, features = ["postgres", "derive", "runtime-tokio", "tls-rustls"] }
```

pgkit depends on sqlx without a runtime or TLS backend, so your application
chooses those through its own sqlx features.

Requires Rust 1.94 or newer.

## Quick start

Describe a table once, with a plain struct:

```rust
use pgkit::PgKit;
use pgkit::query_builder::{Query, WhereOps};
use pgkit::types::DatabaseTable;

#[derive(PgKit)]
#[pgkit(table_name = "geo_countries")]
pub struct CountriesRow {
    #[pgkit(primary_key)]
    pub id: i32,
    pub iso2: String,
    pub name: String,
}

// The derive generated `CountriesTable`, `CountriesColumn` and
// `PartialCountriesRow`. Use them instead of string literals:
let query = CountriesTable::select()
    .columns([CountriesColumn::Id, CountriesColumn::Name])
    .where_eq(CountriesColumn::Iso2, "UG")
    .build();

assert_eq!(
    query.sql(),
    "SELECT geo_countries.id, geo_countries.name FROM geo_countries WHERE geo_countries.iso2 = $1"
);
```

Two equivalent entry points, pick whichever you prefer:
`CountriesTable::select()` (via the `DatabaseTable` trait) pre-fills the table
name, while `Query::select().from(CountriesTable)` also accepts plain strings.
Both exist for `select`, `insert`, `update` and `delete`.

`build()` returns a regular `sqlx::QueryBuilder`, so anything sqlx can do with
one is still available.

## Running a query

Each builder has `build_query_as`, `build_query_scalar` and `build_query`
shortcuts that go straight to sqlx's fetch methods:

```rust,no_run
use pgkit::PgKit;
use pgkit::query_builder::{Query, WhereOps};

#[derive(sqlx::FromRow, PgKit)]
#[pgkit(table_name = "geo_countries")]
pub struct CountriesRow {
    #[pgkit(primary_key)]
    pub id: i32,
    pub iso2: String,
    pub name: String,
}

async fn find_by_iso2(
    pool: &sqlx::PgPool,
    iso2: &str,
) -> Result<Option<CountriesRow>, sqlx::Error> {
    Query::select()
        .from(CountriesTable)
        .all()
        .where_eq(CountriesColumn::Iso2, iso2)
        .build_query_as::<CountriesRow>()
        .fetch_optional(pool)
        .await
}
```

## Query builder

Plain strings work anywhere a table or column is expected, so you can use the
builder without the derive:

```rust
use pgkit::query_builder::{Query, WhereOps};

// SELECT with a join. Conditions chain in call order.
let select = Query::select()
    .from("addresses a")
    .columns(["a.id", "c.name"])
    .inner_join("geo_countries c", |on| on.eq("a.country_id", "c.id"))
    .where_eq("a.owner_id", 42)
    .where_null("a.deleted_at")
    .build();
assert_eq!(
    select.sql(),
    "SELECT a.id, c.name FROM addresses a INNER JOIN geo_countries c ON a.country_id = c.id WHERE a.owner_id = $1 AND a.deleted_at IS NULL"
);

// INSERT with an upsert.
let insert = Query::insert()
    .into("geo_countries")
    .value("iso2", "UG")
    .value("name", "Uganda")
    .on_conflict(["iso2"])
    .do_update(|u| u.set_excluded("name"))
    .returning_all()
    .build();
assert_eq!(
    insert.sql(),
    "INSERT INTO geo_countries (iso2, name) VALUES ($1, $2) ON CONFLICT (iso2) DO UPDATE SET name = EXCLUDED.name RETURNING *"
);

// UPDATE. `set_if_some` skips the column when the value is `None`,
// which suits PATCH-style endpoints.
let new_name: Option<&str> = None;
let update = Query::update()
    .table("geo_countries")
    .set("iso2", "UG")
    .set_if_some("name", new_name)
    .where_eq("id", 1)
    .build();
assert_eq!(update.sql(), "UPDATE geo_countries SET iso2 = $1 WHERE id = $2");

// DELETE.
let delete = Query::delete().from("geo_countries").where_eq("id", 1).build();
assert_eq!(delete.sql(), "DELETE FROM geo_countries WHERE id = $1");
```

The `WHERE` methods cover comparisons, `IN` / `NOT IN`, `ANY` / `ALL`,
`BETWEEN`, `IS NULL`, `EXISTS`, grouping, conditional clauses and raw
fragments:

```rust
use pgkit::query_builder::{Query, WhereOps};

let include_archived = false;

let query = Query::select()
    .from("users")
    .where_eq("status", "active")
    .where_in("role", vec!["admin", "editor"])
    .or_group(|g| g.where_null("deleted_at").where_gt("age", 18))
    .when(include_archived, |b| b.or_where_not_null("archived_at"))
    .build();

assert_eq!(
    query.sql(),
    "SELECT * FROM users WHERE ((status = $1 AND role IN ($2, $3)) OR (deleted_at IS NULL AND age > $4))"
);
```

### Safety guards

`UPDATE` and `DELETE` panic when `.build()` sees an empty `WHERE` clause or
one made entirely of conditions it recognizes as unconditional, such as empty
`NOT IN` lists. Call `.all()` to allow these statements. This guard does not
analyze raw SQL or prove that a predicate restricts the affected rows.

Column and table names passed as strings are inserted into the SQL as written.
Use trusted names and raw SQL fragments. Methods such as `where_eq`, `value`
and `set` bind values; in raw SQL closures, use `push_bind` for values.

Because names go in as written, Postgres lowercases them unless quoted. For a
mixed-case or reserved-word name, `quote_ident!` quotes it at compile time:

```rust
use pgkit::query_builder::{Query, WhereOps};
use pgkit::quote_ident;

let query = Query::select()
    .from(quote_ident!("User"))
    .where_eq(quote_ident!("userName"), "musa")
    .build();

assert_eq!(query.sql(), r#"SELECT * FROM "User" WHERE "userName" = $1"#);
```

`quote_ident!("u", "userName")` renders a qualified `"u"."userName"`. The name
must be a string literal without a `"` in it.

## Pagination

Offset pagination:

```rust
use pgkit::pagination::offset::OffsetPagination;
use pgkit::query_builder::{Query, WhereOps};

let query = Query::select()
    .from("geo_countries")
    .all()
    .where_null("deleted_at")
    .paginate(&OffsetPagination::new(2, 20)) // page 2, 20 rows per page
    .build();

assert_eq!(
    query.sql(),
    "SELECT * FROM geo_countries WHERE deleted_at IS NULL LIMIT $1 OFFSET $2"
);
```

Cursor pagination adds a keyset predicate, an `ORDER BY` with the primary
key as tiebreaker, and a limit for one extra row to detect the next page.
It avoids `OFFSET`; performance depends on indexes and the query. Use it
without a separate ordering, offset or pagination clause:

```rust
use pgkit::PgKit;
use pgkit::ordering::{OrderBy, OrderByDirection};
use pgkit::pagination::cursor::CursorPagination;
use pgkit::query_builder::Query;

#[derive(PgKit)]
#[pgkit(table_name = "geo_countries")]
pub struct CountriesRow {
    #[pgkit(primary_key)]
    pub id: i32,
    pub name: String,
}

fn main() -> Result<(), pgkit::pagination::cursor::CursorPaginationError> {
    // `cursor` is the opaque string a client sends back to get the next page.
    let cursor: Option<String> = None;

    let pagination =
        CursorPagination::<CountriesColumn, i32>::with_validated_column_selection::<()>(
            OrderBy::new(CountriesColumn::Name, OrderByDirection::Asc),
            20,
            cursor,
            None,
        )?;

    let query = Query::select()
        .from(CountriesTable)
        .all_columns_of::<CountriesColumn>()
        .cursor_paginate(&pagination)
        .build();

    assert_eq!(
        query.sql(),
        "SELECT geo_countries.id, geo_countries.name FROM geo_countries ORDER BY geo_countries.name ASC NULLS LAST, geo_countries.id ASC NULLS LAST LIMIT $1"
    );
    Ok(())
}
```

After fetching, `pagination.into_response_parts(rows)` trims the extra row and
returns an optional next-page cursor to `encode()` for the client. Fetched rows
must implement `PaginatedRow`; the derive implements it on the generated partial
row. Select the primary key and ordering column so a next cursor can be built.

Cursors contain URL-safe base64-encoded JSON, without encryption or a signature.
Malformed cursors and known sort-value type mismatches return errors. A change
in sort column, direction or filter fingerprint discards the cursor and restarts
at page one. Filter checks cover only the filters you pass to the pagination
constructor; pgkit does not extract them from the query.

## Retrying transient failures

```rust
use pgkit::errors::RepositoryResult;

struct CountriesRepository;

impl CountriesRepository {
    #[pgkit::retry(tries = 3, backoff = "exponential", delay_ms = 50, idempotent, jitter)]
    async fn count(&self) -> RepositoryResult<i64> {
        // ... run a query
        Ok(0)
    }
}
```

By default, retries cover pool acquisition timeouts and SQLSTATEs `40001`
(serialization failure), `40P01` (deadlock), `53300` (too many connections) and
`57P03` (cannot connect now). I/O, TLS, worker crashes and `08…` connection
errors are retried only with `idempotent`. Constraint violations and missing
rows are not retried.

Every retry repeats the whole function body. Earlier successful statements or
external side effects are not undone by pgkit. Make the whole body safe to
repeat, for example by using a transaction for related database writes.
`idempotent` is your assertion of safety; pgkit does not verify it.

The function must be `async` and return `RepositoryResult<T>`. It also works
inside `#[async_trait]` impls. Backoff sleeps use `tokio::time::sleep`, so the
`retry` feature requires a Tokio runtime.

## Embedded migrations and seeds

Enable `db-init` to embed migrations and seeds:

```toml
pgkit = { version = "0.4", features = ["db-init"] }
```

Each macro takes a scan root relative to the calling crate's `Cargo.toml`.
It finds descendant folders named `migrations` or `seeds` and embeds the
`.sql` files directly inside them. Pass the parent of these folders, not the
`migrations` or `seeds` folder itself. Subfolders inside a matching script
folder are not scanned for that kind of script.

```text
src/
├── billing/
│   └── db/
│       ├── migrations/
│       │   └── 20240101120000_create_invoices.up.sql
│       └── seeds/
│           └── 001_default_plans.sql
└── users/
    └── db/
        └── migrations/
            └── 20240102090000_create_users.up.sql
```

```rust,ignore
let pool = sqlx::PgPool::connect(&database_url).await?;

pgkit::migrator!("src").run(&pool).await?; // an sqlx::migrate::Migrator
pgkit::seeder!("src").run(&pool).await?;   // a pgkit::db_init::seeding::Seeder
```

Migration files use `<14-digit version>_<name>.up.sql`. The macro checks file
names and duplicate versions within its scan, then sorts by version. It accepts
only up migrations; `.down.sql` files fail the build. SQL syntax is checked by
PostgreSQL when run. Files starting with `-- no-transaction` run outside a
transaction; otherwise each migration runs in its own transaction, not one
transaction for the collection. SQLx tracks versions and SQL checksums in
`_sqlx_migrations`. The checksums match SQLx's for the same SQL content.

Seeds run in path order, without version tracking. Prefix names with numbers
to control order within a folder. Every seed runs on every call, so make
repeated runs safe:

```sql
INSERT INTO plans (code, name)
SELECT 'standard', 'Standard'
WHERE NOT EXISTS (SELECT 1 FROM plans);
```

`Seeder::run` attempts every script in slice order, logs failures, and returns
them together in `SeedErrors`. It does not wrap the collection in a transaction.

### Merging collections from multiple crates

`migrations!` returns a `Vec<sqlx::migrate::Migration>`; `seeds!` returns a
`&'static [Seed]`. A dependency can export these collections. It embeds its
own SQL when compiled, so the application does not need to locate its files:

```rust,ignore
// In the library crate — "db" is relative to *its* Cargo.toml.
pub fn migrations() -> Vec<sqlx::migrate::Migration> {
    pgkit::migrations!("db")
}

pub const SEEDS: &[pgkit::db_init::seeding::Seed] = pgkit::seeds!("db");
```

Merge dependency and application migrations before running them.
`merge_migrations` sorts by version and rejects duplicate versions —
naming the colliding version — before any SQL runs:

```rust,ignore
let migrator = pgkit::db_init::migrating::merge_migrations([
    billing::migrations(),
    users::migrations(),
    pgkit::migrations!("src"),
])?;
migrator.run(&pool).await?;
```

Versions must be globally unique. The merged migrator sorts by version, so
append order does not control execution: coordinate versions when migrations
depend on another crate's schema. Each migration keeps its own history entry.

For one combined seeder, store the merged seeds in a `OnceLock` to satisfy
`Seeder::new`'s `&'static [Seed]` requirement:

```rust,ignore
use std::sync::OnceLock;
use pgkit::db_init::seeding::{Seed, Seeder};

static ALL_SEEDS: OnceLock<Vec<Seed>> = OnceLock::new();

let seeds = ALL_SEEDS.get_or_init(|| {
    let mut all = billing::SEEDS.to_vec();
    all.extend_from_slice(users::SEEDS);
    all.extend_from_slice(pgkit::seeds!("src"));
    all
});
Seeder::new(seeds.as_slice()).run(&pool).await?;
```

Alternatively, run separate seeders in order. Using `?` stops later collections
when an earlier one fails; a combined seeder attempts all scripts.

Published dependencies must include their `.sql` files in the package. Keep
previously applied migrations in the merged collection: missing versions fail
SQLx's default validation. `set_ignore_missing(true)` disables that missing-version
check; it does not undo a removed dependency's schema.

Embedded file edits trigger recompilation. To also detect added or removed
files, add a `build.rs` in each crate that scans SQL, watching its scan root:

```rust,ignore
fn main() {
    println!("cargo::rerun-if-changed=src");
}
```

## Errors

`RepositoryError` converts from `sqlx::Error` and sorts Postgres failures into
variants you can match on:

```rust
use pgkit::errors::{RepositoryError, RepositoryResult};

struct Vendor;

enum AppError {
    DuplicateName,
    Repository(RepositoryError),
}

fn map_create_result(result: RepositoryResult<Vendor>) -> Result<Vendor, AppError> {
    match result {
        Ok(vendor) => Ok(vendor),
        Err(RepositoryError::UniqueViolation { constraint: Some(c), .. })
            if c == "uq_vendors_name" =>
        {
            Err(AppError::DuplicateName)
        }
        Err(e) => Err(AppError::Repository(e)),
    }
}
```

## Full-text search

`build_tsquery` splits text on whitespace, strips tsquery operator characters,
adds `:*` to each remaining term, and joins up to
[`MAX_TERMS`](https://docs.rs/pgkit/latest/pgkit/full_text_search/constant.MAX_TERMS.html)
terms with
`&`. It returns `None` if no terms remain. Bind the result as a SQL parameter:

```rust
use pgkit::full_text_search::build_tsquery;

assert_eq!(build_tsquery("red shoe").as_deref(), Some("red:* & shoe:*"));
assert_eq!(build_tsquery("   "), None);
```

## Connection pool

`pool::get` opens a `PgPool` from your `PgConnectOptions`, and a `PoolConfig`.

```rust,no_run
use pgkit::pool::{self, PoolConfig};
use sqlx::postgres::PgConnectOptions;

async fn connect() -> Result<sqlx::PgPool, sqlx::Error> {
    let options = PgConnectOptions::new()
        .host("localhost")
        .port(5432)
        .username("app")
        .password("password")
        .database("app");

    pool::get(options, PoolConfig {
        max_connections: 20,
        ..PoolConfig::default()
    })
    .await
}
```

Defaults and the minimum target for open connections are documented on the
corresponding
[`PoolConfig`](https://docs.rs/pgkit/latest/pgkit/pool/struct.PoolConfig.html)
field. Session timeouts limit long statements and idle transactions; they do
not prevent pool exhaustion.

`Duration::ZERO` disables either session timeout. For migrations that need
longer than the defaults, adjust or disable them:

```rust
use std::time::Duration;
use pgkit::pool::PoolConfig;

let config = PoolConfig {
    max_connections: 2,
    statement_timeout: Duration::ZERO,
    idle_tx_timeout: Duration::ZERO,
    ..PoolConfig::default()
};
```

## Feature flags

Defaults enable `partial-row`, `pagination`, `repository-error` and `retry`.
`pagination` enables both pagination features; `cursor-pagination` also enables
`serde`. `db-init` is opt-in.

| Feature             | Provides                                                                                       |
| ------------------- | ---------------------------------------------------------------------------------------------- |
| `partial-row`       | `try_get_option`, and with `cursor-pagination`, `#[derive(PgKit)]`                             |
| `offset-pagination` | `pagination::offset`                                                                           |
| `cursor-pagination` | `pagination::cursor`                                                                           |
| `pagination`        | Both pagination features                                                                       |
| `repository-error`  | `errors::RepositoryError`; its `CursorPagination` variant only exists with `cursor-pagination` |
| `retry`             | `#[pgkit::retry]` and `retry::run` (needs Tokio)                                               |
| `serde`             | `Serialize` / `Deserialize` for `OrderByDirection` and similar                                 |
| `db-init`           | `migrations!`, `migrator!`, `seeds!`, `seeder!` and `db_init`                                  |

The query builder, `ordering`, `projection`, `pool` and `full_text_search`
are always available. To use only the query builder:

```toml
pgkit = { version = "0.4", default-features = false }
```

## Crates in this repository

| Crate                | Purpose                                           |
| -------------------- | ------------------------------------------------- |
| `pgkit`              | The library. This is the one you depend on.       |
| `pgkit-macros`       | The procedural macros, re-exported by `pgkit`.    |
| `pgkit-macros-tests` | Compile-fail tests for the macros. Not published. |

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this work by you, as defined in the Apache-2.0 license, shall
be dual licensed as above, without any additional terms or conditions.
