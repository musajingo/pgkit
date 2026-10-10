//! Emits `PaginatedRow<Id, Column>` for the partial row.
//!
//! # What this emits
//!
//! For
//!
//! ```ignore
//! #[derive(PgKit)]
//! #[pgkit(table_name = "geo_countries")]
//! pub struct CountryRow {
//!     #[pgkit(primary_key)] pub id: i32,
//!     pub name: String,
//!     pub region: Option<String>,
//!     #[pgkit(json)] pub metadata: sqlx::types::Json<serde_json::Value>,
//! }
//! ```
//!
//! this emits roughly:
//!
//! ```ignore
//! impl ::pgkit::pagination::cursor::PaginatedRow<i32, CountryColumn> for PartialCountryRow {
//!     fn row_id(&self) -> Result<i32, CursorPaginationError> {
//!         self.id.clone().ok_or(CursorPaginationError::IdColumnWasNotProjected)
//!     }
//!
//!     fn ordering_column_value(
//!         &self,
//!         column: CountryColumn,
//!     ) -> Result<Option<CursorValue>, CursorPaginationError> {
//!         match column {
//!             CountryColumn::Id => {
//!                 let value = self.id.clone().ok_or_else(||
//!                     CursorPaginationError::OrderingColumnNotProjected {
//!                         column: column.to_string(),
//!                     })?;
//!                 Ok(Some(CursorValue::from(value)))
//!             }
//!             CountryColumn::Name => {
//!                 let value = self.name.clone().ok_or_else(||
//!                     CursorPaginationError::OrderingColumnNotProjected {
//!                         column: column.to_string(),
//!                     })?;
//!                 Ok(Some(CursorValue::from(value)))
//!             }
//!             CountryColumn::Region => {
//!                 let value = self.region.clone().ok_or_else(||
//!                     CursorPaginationError::OrderingColumnNotProjected {
//!                         column: column.to_string(),
//!                     })?;
//!                 match value {
//!                     Some(value) => Ok(Some(CursorValue::from(value))),
//!                     None => Ok(None),
//!                 }
//!             }
//!             CountryColumn::Metadata => {
//!                 let value = self.metadata.clone().ok_or_else(||
//!                     CursorPaginationError::OrderingColumnNotProjected {
//!                         column: column.to_string(),
//!                     })?;
//!                 let value = serde_json::to_value(&value)
//!                     .map(CursorValue::Json)
//!                     .map_err(|error| CursorPaginationError::InternalError {
//!                         message: format!("failed to serialize cursor ordering column \"{}\"", column),
//!                         source: error.into(),
//!                     })?;
//!                 Ok(Some(value))
//!             }
//!         }
//!     }
//! }
//! ```
//!
//! `ordering_column_value` matches every column in the enum. Whether a column
//! supports the keyset query's comparisons is checked by PostgreSQL at runtime.
//!
//! Each column's non-nullable Rust type needs `From<T> for CursorValue`, unless
//! marked `#[pgkit(json)]`. JSON fields use `serde_json::to_value`; serialization
//! errors return `CursorPaginationError::InternalError` with the column name
//! and original error. Custom enum types need their own `From`
//! implementation. These requirements disappear when this impl is skipped.
//!
//! Two branches in the per-column arm:
//!
//! - **Non-nullable field** (`name: Option<String>` on the partial,
//!   source was `String`): clone, convert to `CursorValue`, and wrap in
//!   `Some` because a missing projection is the only way to get `None`.
//! - **Nullable field** (`region: Option<Option<String>>` on the
//!   partial, source was `Option<String>`): clone, then convert the
//!   inner option's value to `CursorValue` as-is, so SQL `NULL` (the
//!   inner `None`) flows through to the cursor.
//!
//! In both branches the *outer* `Option` answers "was this column
//! projected?": a `None` there becomes `Err(OrderingColumnNotProjected)`.
//!
//! Skipped in two cases:
//!
//! - `skip(paginated_row)`: explicit opt-out. The caller wants the
//!   column enum and partial but never mints cursors (e.g. for offset
//!   pagination or plain `ORDER BY`).
//! - the partial is skipped: `PaginatedRow` is implemented *on* the
//!   partial, so there's no type to implement it for.

use proc_macro2::TokenStream;
use quote::quote;

use super::model::Model;

pub(crate) fn emit(model: &Model) -> TokenStream {
    // `PaginatedRow` is implemented *on* the partial, so a skipped partial
    // leaves nothing to implement it for; `skip(paginated_row)` is the
    // explicit opt-out. Otherwise the impl is always emitted: every
    // column is a valid cursor sort key.
    if model.skip.paginated_row || model.skip.partial {
        return TokenStream::new();
    }

    let partial_ident = &model.names.partial;
    let columns_ident = &model.names.columns;

    let pk_field = model.primary_key_field();
    let pk_ident = &pk_field.ident;
    let pk_ty = &pk_field.ty;

    // One arm per column; the match is exhaustive over the full column enum.
    let value_arms = model.fields.iter().map(|f| {
        let variant = &f.variant_ident;
        let field_ident = &f.ident;

        // A `#[pgkit(json)]` column carries its cursor value as
        // `CursorValue::Json`; the Rust type need only be `Serialize`, not
        // `Into<CursorValue>` (JSON/JSONB types rarely have a `From` impl, and
        // a foreign newtype can't be given one under the orphan rule).
        let cursor_value = if f.is_json {
            quote! {
                ::pgkit::__private::serde_json::to_value(&value)
                    .map(::pgkit::pagination::cursor::CursorValue::Json)
                    .map_err(|error| ::pgkit::pagination::cursor::CursorPaginationError::InternalError {
                        message: ::std::format!("failed to serialize cursor ordering column \"{}\"", column),
                        source: error.into(),
                    })?
            }
        } else {
            quote! { ::pgkit::pagination::cursor::CursorValue::from(value) }
        };

        // The outer option records projection. For nullable fields, the inner
        // option preserves SQL NULL without attempting JSON serialization.
        let result = if f.option_inner.is_some() {
            quote! {
                match value {
                    ::core::option::Option::Some(value) =>
                        ::core::result::Result::Ok(::core::option::Option::Some(#cursor_value)),
                    ::core::option::Option::None =>
                        ::core::result::Result::Ok(::core::option::Option::None),
                }
            }
        } else {
            quote! {
                ::core::result::Result::Ok(::core::option::Option::Some(#cursor_value))
            }
        };

        quote! {
            #columns_ident::#variant => {
                let value = self
                    .#field_ident
                    .clone()
                    .ok_or_else(|| ::pgkit::pagination::cursor::CursorPaginationError::OrderingColumnNotProjected {
                        column: column.to_string(),
                    })?;
                #result
            }
        }
    });

    quote! {
        #[automatically_derived]
        impl ::pgkit::pagination::cursor::PaginatedRow<#pk_ty, #columns_ident>
            for #partial_ident
        {
            fn row_id(
                &self,
            ) -> ::core::result::Result<
                #pk_ty,
                ::pgkit::pagination::cursor::CursorPaginationError,
            > {
                self.#pk_ident
                    .clone()
                    .ok_or(::pgkit::pagination::cursor::CursorPaginationError::IdColumnWasNotProjected)
            }

            fn ordering_column_value(
                &self,
                column: #columns_ident,
            ) -> ::core::result::Result<
                ::core::option::Option<::pgkit::pagination::cursor::CursorValue>,
                ::pgkit::pagination::cursor::CursorPaginationError,
            > {
                match column {
                    #(#value_arms)*
                }
            }
        }
    }
}
