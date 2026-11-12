//! # TV Studio
//!
//! Среда визуальной разработки текстовых приложений на tvision-rs:
//! дизайнер форм, инспектор свойств, модель приложения (меню, строка
//! состояния, команды), генерация кода на Rust, редактор кода с
//! подсветкой и сборка через cargo.
//!
//! | Модуль | Что в нём |
//! |--------|-----------|
//! | [`model`] | модель: формы, элементы, меню, команды, проект |
//! | [`props`] | свойства элементов для инспектора |
//! | [`builder`] | живой диалог tvision-rs по модели формы |
//! | [`codegen`] | генерация кода на Rust |
//! | [`designer`] | дизайнер форм (окно и холст) |
//! | [`inspector`] | инспектор свойств |
//! | [`toolwins`] | палитра, дерево проекта, сообщения |
//! | [`appdesign`] | редакторы меню, строки состояния, команд |
//! | [`codeedit`], [`highlight`] | редактор кода и подсветка |
//! | [`runner`] | запуск cargo в фоне |
//! | [`dialogs`] | прочие диалоги и настройки среды |
//! | [`toml`], [`json`] | формат файлов проекта и форм (TOML; старые — JSON) |
//! | [`formextract`] | извлечение форм из диалогов в коде |
//! | [`importer`] | импорт готовой программы на tvision-rs |
//! | [`ide`] | сама среда: меню и обработчик команд |

pub mod appdesign;
pub mod builder;
pub mod cmds;
pub mod codeedit;
pub mod codegen;
pub mod designer;
pub mod dialogs;
pub mod formextract;
pub mod highlight;
pub mod ide;
pub mod importer;
pub mod inspector;
pub mod json;
pub mod lsp;
pub mod model;
pub mod props;
pub mod runner;
pub mod rustfmt;
pub mod toml;
pub mod toolwins;
pub mod words;
