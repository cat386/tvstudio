//! Общий код примеров с JSON: разбор ([`parser`]) и превращение в таблицу
//! ([`table`]). Подключается в примере так:
//!
//! ```ignore
//! #[path = "json/mod.rs"]
//! mod json;
//! use json::*;
//! ```
//!
//! Это не отдельный пример: в каталоге нет `main.rs`, и cargo его не собирает
//! сам по себе.

// Каждый пример берёт только то, что ему нужно; остальное не должно
// вызывать предупреждений «не используется».
#![allow(dead_code)]

pub mod parser;
pub mod table;

pub use parser::*;
// `json_parse` таблицу не строит.
#[allow(unused_imports)]
pub use table::*;
