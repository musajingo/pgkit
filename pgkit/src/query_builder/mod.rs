//! PostgreSQL query builders. Bring [`WhereOps`] into scope for WHERE methods.
//!
//! Value methods bind SQL parameters. Table names, column names and raw SQL
//! are inserted as supplied and must be trusted. Generated table and column
//! types provide Rust metadata; SQL validity is checked when the database runs it.

pub mod operators;

mod delete_builder;
mod exec;
mod insert_builder;
mod ops;
mod query;
mod select_builder;
mod update_builder;
mod where_builder;

pub use delete_builder::DeleteBuilder;
pub use exec::{BuildQuery, BuildQueryAs, BuildQueryScalar};
pub use insert_builder::{
    ConflictTarget, InsertBuilder, InsertRow, OnConflict, OnConflictUpdate, Returning,
};
pub use ops::{HasWhere, WhereOps};
pub use query::Query;
pub use select_builder::{
    CteBuilder, Distinct, JoinKind, JoinOn, NullsOrder, OrderItem, SelectBuilder,
};
pub use update_builder::UpdateBuilder;
pub use where_builder::WhereBuilder;
