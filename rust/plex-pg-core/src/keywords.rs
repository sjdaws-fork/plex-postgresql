/// Module: keywords
///
/// Rewrites SQLite-specific keywords/constructs to PostgreSQL equivalents:
///   BEGIN IMMEDIATE / DEFERRED / EXCLUSIVE -> plain BEGIN
///   GLOB '*foo*'        -> ILIKE '%foo%'  (via pre-parse string normalisation)
///   IN ()               -> IN (SELECT -1 WHERE FALSE)
///   GROUP BY NULL       -> remove GROUP BY
///   sqlite_master / sqlite_schema -> information_schema subquery
///   INDEXED BY idx      -> removed  (via pre-parse string normalisation)
///   ORDER BY rowid      -> removed when querying sqlite_master
use sqlparser::ast::helpers::attached_token::AttachedToken;
use sqlparser::ast::*;
use sqlparser::tokenizer::Span;
#[cfg(test)]
use std::sync::atomic::AtomicI8;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::env_utils;

mod ast_transform;
mod preprocess;

use ast_transform::{transform_expr, transform_query};
use preprocess::preprocess_sql;

const STRICT_PRAGMA_ERROR_SQL: &str = "__STRICT_PRAGMA_ERROR__";
static PRAGMA_TOTAL: AtomicU64 = AtomicU64::new(0);
static PRAGMA_MAPPED_SET: AtomicU64 = AtomicU64::new(0);
static PRAGMA_MAPPED_READ: AtomicU64 = AtomicU64::new(0);
static PRAGMA_NOOP_SET: AtomicU64 = AtomicU64::new(0);
static PRAGMA_STRIPPED: AtomicU64 = AtomicU64::new(0);
static PRAGMA_STRICT_FAIL: AtomicU64 = AtomicU64::new(0);
#[cfg(test)]
static TEST_STRICT_PRAGMA_OVERRIDE: AtomicI8 = AtomicI8::new(-1);

pub fn transform(stmt: &mut Statement) {
    match stmt {
        Statement::StartTransaction { modifier, .. } => {
            *modifier = None;
        }
        Statement::Query(q) => transform_query(q),
        Statement::Insert(ins) => {
            if let Some(src) = &mut ins.source {
                transform_query(src);
            }
        }
        Statement::Update(u) => {
            if let Some(sel) = &mut u.selection {
                transform_expr(sel);
            }
        }
        Statement::Delete(d) => {
            if let Some(sel) = &mut d.selection {
                transform_expr(sel);
            }
        }
        _ => {}
    }
}

pub fn preprocess(sql: &str) -> String {
    preprocess_sql(sql)
}

#[cfg(test)]
#[path = "keywords_tests.rs"]
mod keywords_tests;
