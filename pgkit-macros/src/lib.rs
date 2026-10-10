use proc_macro::TokenStream;

mod db_init;
mod pg_kit;
mod retry;

/// Derives a table type, a column enum, and a partial row from a row struct.
///
/// Generates a `DatabaseTable` unit struct, a `DatabaseTableColumn` enum,
/// and a partial row with `sqlx::FromRow` and `PaginatedRow` implementations.
/// Does not create a table, check the database schema, or implement `FromRow`
/// on the original struct. Derive `sqlx::FromRow` separately when needed.
///
/// Requires a concrete struct with named fields, one primary-key field and
/// a table name. Available through `pgkit` with both `partial-row` and
/// `cursor-pagination` enabled.
///
/// # Container attributes
///
/// On the struct, inside `#[pgkit(...)]`:
///
/// | Key | Purpose |
/// | --- | --- |
/// | `table_name = "..."` | Required SQL table name. |
/// | `table = "Ident"` | Table type name; defaults to the struct name minus trailing `Row`, plus `Table`. |
/// | `columns = "Ident"` | Column enum name; defaults to the struct name minus trailing `Row`, plus `Column`. |
/// | `partial = "Ident"` | Partial row name; defaults to `Partial` plus the full struct name. |
/// | `skip(partial, paginated_row)` | Omit either or both; skipping the partial also omits its pagination impl. Table and columns cannot be skipped. |
///
/// # Field attributes
///
/// | Attribute | Effect |
/// | --- | --- |
/// | `#[pgkit(primary_key)]` | Required on exactly one field; selects the PK column and pagination ID type. |
/// | `#[pgkit(rename = "...")]` | Sets the SQL column name and partial-row lookup name. |
/// | `#[pgkit(json)]` | Converts cursor values through `serde_json::to_value` instead of `CursorValue::from`. |
///
/// Keys can share one attribute: `#[pgkit(primary_key, rename = "country_id")]`.
///
/// SQL column names use the field name or `rename` value for both the enum
/// and partial-row lookup. Names are emitted without SQL quoting; supply
/// trusted names appropriate for the database schema.
///
/// All fields need SQLx Postgres type information. Generated partial fields
/// also need `Debug`, `Clone` and SQLx decoding. Nullable fields become
/// `Option<Option<T>>`: the outer option records projection, the inner SQL NULL.
/// Cursor extraction requires `From<T> for CursorValue` for each field's
/// non-nullable type, unless marked `json` or pagination is skipped. A `json`
/// field needs `Serialize`; serialization failures return
/// `CursorPaginationError::InternalError` with the column name and original error.
///
/// Other attributes (`#[sqlx(...)]`, `#[serde(...)]`, `#[doc = ...]`) are
/// ignored by this derive. If a column needs renaming in `sqlx`'s derive
/// *and* `PgKit`, supply both `#[sqlx(rename = "x")]` and
/// `#[pgkit(rename = "x")]`.
///
/// # Example
///
/// ```
/// use pgkit::PgKit;
/// use pgkit::types::DatabaseTable;
///
/// #[derive(PgKit)]
/// #[pgkit(table_name = "geo_countries")]
/// pub struct CountryRow {
///     #[pgkit(primary_key)]
///     pub id: i32,
///
///     pub iso2: String,
///
///     pub name: String,
///
///     pub region: Option<String>,
///
///     #[pgkit(rename = "deleted_at")]
///     pub deleted: Option<chrono::DateTime<chrono::Utc>>,
/// }
///
/// // Emits:
/// //   pub struct CountryTable;         (impl DatabaseTable: CountryTable::select()/insert()/update()/delete())
/// //   pub enum   CountryColumn { Id, Iso2, Name, Region, Deleted } (impl DatabaseTableColumn)
/// //   pub struct PartialCountryRow { id: Option<i32>, …, deleted: Option<Option<DateTime<Utc>>> }
/// //   impl sqlx::FromRow<PgRow>             for PartialCountryRow
/// //   impl PaginatedRow<i32, CountryColumn> for PartialCountryRow
///
/// assert_eq!(CountryTable::NAME, "geo_countries");
/// assert_eq!(CountryTable::select().all().build().sql(), "SELECT * FROM geo_countries");
/// assert_eq!(CountryColumn::Deleted.to_string(), "deleted_at");
/// assert!(PartialCountryRow::default().id.is_none());
/// ```
#[proc_macro_derive(PgKit, attributes(pgkit))]
pub fn derive_pg_kit(input: TokenStream) -> TokenStream {
    pg_kit::expand(input)
}

/// Wraps an async function returning `RepositoryResult<T>` in a retry loop.
///
/// Expands to `pgkit::retry::run(policy, || async { body })`: the body
/// re-executes while the error is retryable and attempts remain; permanent
/// errors (constraint violations, missing rows, …) return immediately.
/// Between attempts a `tracing` warning is logged and the backoff delay
/// slept.
///
/// # Retry safety
///
/// Defaults retry pool acquisition timeouts and SQLSTATEs `40001`, `40P01`,
/// `53300` and `57P03`. I/O, TLS, worker crashes and `08…` connection errors
/// require `idempotent`. Constraint violations and missing rows are not retried.
///
/// Every retry repeats the whole body. Earlier successful statements and
/// external side effects are not undone. Make the whole body safe to repeat;
/// `idempotent` is your assertion, not a check performed by pgkit. Backoff
/// sleeps require a Tokio runtime with time enabled.
///
/// # Arguments
///
/// | Key            | Default         | Meaning                                              |
/// |----------------|-----------------|------------------------------------------------------|
/// | `tries`        | `3`             | Total attempts, including the first execution.       |
/// | `backoff`      | `"linear"`      | `"fixed"`, `"linear"`, or `"exponential"`.           |
/// | `delay_ms`     | `100`           | Base delay between attempts, in milliseconds.        |
/// | `max_delay_ms` | `10000`         | Upper bound any computed delay is clamped to.        |
/// | `idempotent`   | off             | Also retry ambiguous mid-statement failures.         |
/// | `jitter`       | off             | Randomize each delay by ×[0.5, 1.5).                 |
///
/// # Example
///
/// ```
/// # use pgkit::errors::RepositoryResult;
/// # use tracing::instrument;
/// # struct CountriesRow;
/// # struct CountriesRepository;
/// # impl CountriesRepository {
/// #[instrument(skip(self))]
/// #[pgkit::retry(tries = 3, backoff = "exponential", delay_ms = 50, idempotent, jitter)]
/// async fn view(&self, id: i32) -> RepositoryResult<CountriesRow> {
///     // ...
/// #   Ok(CountriesRow)
/// }
/// # }
/// ```
///
/// # Requirements
///
/// - The function must return `RepositoryResult<T>`; the retry engine
///   classifies the error via `RepositoryError`.
/// - Accepted shapes: a plain `async fn`, or a method inside an
///   `#[async_trait]` impl (the macro detects async_trait's desugaring and
///   wraps the retry loop inside the boxed future).
/// - The body re-executes on retry, so it may only borrow its arguments;
///   moving an owned non-`Copy` argument into the body fails to compile.
///   In `#[async_trait]` impls this extends to owned non-`Copy` parameters
///   themselves (async_trait moves them into the future). Early `return`s
///   inside the body are preserved (they end the attempt).
#[proc_macro_attribute]
pub fn retry(attr: TokenStream, item: TokenStream) -> TokenStream {
    retry::expand(attr, item)
}

/// Embeds a project's migrations as a `Vec<sqlx::migrate::Migration>`.
///
/// Takes the scan root as a string literal, relative to `CARGO_MANIFEST_DIR`,
/// and finds descendant folders named `migrations`. Only `.sql` files directly
/// inside those folders are collected. Pass their parent as the scan root;
/// the scan root itself is not treated as a migration folder:
///
/// ```text
/// src/
/// ├── billing/
/// │   └── db/migrations/20240101120000_create_invoices.up.sql
/// └── users/
///     └── db/migrations/20240102090000_create_users.up.sql
/// ```
///
/// Each file must be named `<14-digit version>_<name>.up.sql` (the version is
/// the timestamp `sqlx migrate add` generates); versions must be unique across
/// the scan. Down-migration files are not accepted. SQL syntax is not checked.
/// A file starting with `-- no-transaction` runs outside a transaction;
/// otherwise each migration runs in its own transaction.
/// SQL is embedded with `include_str!`; no SQL files are needed at runtime.
/// Checksums match SQLx's for the same SQL content.
///
/// Merge the returned vectors to compose collections from dependencies.
/// Check version uniqueness across the merge before constructing a migrator.
/// Enable `pgkit`'s `db-init` feature in calling crates.
///
/// To detect added or removed files, have the crate's `build.rs` watch the
/// scan root, e.g. `println!("cargo::rerun-if-changed=src");`.
#[proc_macro]
pub fn migrations(input: TokenStream) -> TokenStream {
    db_init::migrations(input)
}

/// [`migrations!`] wrapped in a ready-to-run `sqlx::migrate::Migrator`.
///
/// ```ignore
/// pgkit::migrator!("src").run(&pool).await?;
/// ```
#[proc_macro]
pub fn migrator(input: TokenStream) -> TokenStream {
    db_init::migrator(input)
}

/// Embeds a project's seeds as a `&'static [pgkit::db_init::seeding::Seed]`.
///
/// Takes the scan root as a string literal, relative to `CARGO_MANIFEST_DIR`,
/// and finds descendant folders named `seeds`. Only `.sql` files directly
/// inside those folders are collected; pass their parent as the scan root.
/// No filename format is enforced. Seeds are sorted by path; numbered prefixes
/// control order within a folder. The expression can be used in a `const`
/// or `static`. Enable `pgkit`'s `db-init` feature in calling crates.
///
/// The same `build.rs` note as [`migrations!`] applies.
#[proc_macro]
pub fn seeds(input: TokenStream) -> TokenStream {
    db_init::seeds(input)
}

/// [`seeds!`] wrapped in a ready-to-run `pgkit::db_init::seeding::Seeder`.
///
/// ```ignore
/// pgkit::seeder!("src").run(&pool).await?;
/// ```
#[proc_macro]
pub fn seeder(input: TokenStream) -> TokenStream {
    db_init::seeder(input)
}
