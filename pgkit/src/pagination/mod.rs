//! Offset and cursor pagination.
//!
//! Both styles share one page-size cap: a page never exceeds 256 rows,
//! whatever limit the caller requests. `OffsetPagination::new` clamps on
//! construction; `CursorPagination::effective_limit` caps on read.

mod defaults;

#[cfg(feature = "offset-pagination")]
pub mod offset;

#[cfg(feature = "cursor-pagination")]
pub mod cursor;
