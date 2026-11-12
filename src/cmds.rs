//! Команды и контексты справки среды.
//!
//! Команды до 255 можно запрещать (`disable_command`) — так выключаются
//! пункты меню, когда нет проекта или формы. Команды выше 255 разрешены
//! всегда.

// --- Сборка (запрещаются без проекта) ---
pub const CM_CHECK: u16 = 140;
pub const CM_BUILD: u16 = 141;
pub const CM_RUN: u16 = 142;
pub const CM_TEST: u16 = 143;
pub const CM_CLEAN: u16 = 144;
pub const CM_STOP: u16 = 145;
pub const CM_GENERATE: u16 = 146;
/// Параметры сборки (профиль, features, платформа…) и сборка с ними.
pub const CM_BUILD_OPTS: u16 = 147;

// --- Формы ---
pub const CM_NEW_FORM: u16 = 150;
pub const CM_DS_PROPS: u16 = 151;
pub const CM_FORM_TEST: u16 = 152;
pub const CM_FORM_CODE: u16 = 153;
pub const CM_DS_DUP: u16 = 154;
pub const CM_GRID: u16 = 155;
pub const CM_TABNUM: u16 = 156;
pub const CM_DS_FRONT: u16 = 157;
pub const CM_DS_BACK: u16 = 158;
pub const CM_DS_FORWARD: u16 = 159;
pub const CM_DS_BACKWARD: u16 = 160;
/// Выравнивание: `CM_ALIGN_BASE + Align as u16`.
pub const CM_ALIGN_BASE: u16 = 161;
/// Добавить элемент: `CM_INS_BASE + номер вида в Kind::ALL`.
pub const CM_INS_BASE: u16 = 170;

// --- Модель приложения и проект ---
pub const CM_EDIT_MENU: u16 = 190;
pub const CM_EDIT_STATUS: u16 = 191;
pub const CM_EDIT_COMMANDS: u16 = 192;
pub const CM_APP_OPTIONS: u16 = 193;
pub const CM_CLOSE_PROJECT: u16 = 194;
pub const CM_REMOVE_FORM: u16 = 195;
pub const CM_OPEN_FORM: u16 = 196;
pub const CM_FIND_FILES: u16 = 197;
pub const CM_GOTO: u16 = 198;
pub const CM_NEXT_ERR: u16 = 199;
pub const CM_PREV_ERR: u16 = 200;
pub const CM_RENAME_FORM: u16 = 201;
pub const CM_EXTRACT_FORMS: u16 = 202;

// --- Сервер LSP (rust-analyzer); разрешены, когда открыт редактор кода ---
/// Дополнение (Ctrl+Space; само — после `.` и `::`). Info::Str — символ-триггер.
pub const CM_COMPLETE: u16 = 203;
/// Перейти к определению (F12).
pub const CM_GOTO_DEF: u16 = 204;
/// Найти ссылки (Shift+F12).
pub const CM_FIND_REFS: u16 = 205;
/// Вернуться назад после перехода (Alt+←).
pub const CM_NAV_BACK: u16 = 206;
/// Все диагностики сервера — в окно сообщений.
pub const CM_DIAGNOSTICS: u16 = 207;
/// Быстрые исправления под курсором (Alt+Enter, Ctrl+.).
pub const CM_CODE_ACTIONS: u16 = 208;

// --- Всегда разрешённые ---
pub const CM_NEW_PROJECT: u16 = 1100;
pub const CM_OPEN_PROJECT: u16 = 1101;
pub const CM_RECENT: u16 = 1102;
pub const CM_SHOW_PALETTE: u16 = 1103;
pub const CM_SHOW_INSPECTOR: u16 = 1104;
pub const CM_SHOW_PROJECT: u16 = 1105;
pub const CM_SHOW_MESSAGES: u16 = 1106;
pub const CM_LAYOUT: u16 = 1107;
pub const CM_OPTIONS: u16 = 1108;
pub const CM_ABOUT: u16 = 1109;
pub const CM_KEYS: u16 = 1110;
pub const CM_IMPORT: u16 = 1111;
/// Перезапустить сервер LSP.
pub const CM_LSP_RESTART: u16 = 1112;
/// Окно «Ошибки и предупреждения» (Ctrl+F8).
pub const CM_SHOW_PROBLEMS: u16 = 1113;

// --- Внутренние ---
/// Сообщения: перейти к месту (Info::Id списка или Info::Int строки).
pub const CM_MSG_GOTO: u16 = 1120;
/// Дерево проекта: открыть узел (Info::Id дерева).
pub const CM_PROJ_OPEN: u16 = 1121;
/// Палитра: добавить выбранный компонент (Info::Id списка).
pub const CM_PAL_INSERT: u16 = 1122;
/// Окно ошибок: перейти к месту замечания под курсором.
pub const CM_PROB_GOTO: u16 = 1123;
/// Редактор списков (меню, строка состояния, команды).
pub const CM_LE_ADD: u16 = 1200;
pub const CM_LE_SUB: u16 = 1201;
pub const CM_LE_SEP: u16 = 1202;
pub const CM_LE_EDIT: u16 = 1203;
pub const CM_LE_DEL: u16 = 1204;
pub const CM_LE_UP: u16 = 1205;
pub const CM_LE_DOWN: u16 = 1206;
pub const CM_LE_LEFT: u16 = 1207;
pub const CM_LE_RIGHT: u16 = 1208;
/// Новый проект: кнопка «…» выбора каталога.
pub const CM_BROWSE: u16 = 1210;

// --- Контексты справки (они же выбирают подсказки строки состояния) ---
pub const HC_DESIGNER: u16 = 1000;
pub const HC_EDITOR: u16 = 1001;
pub const HC_INSPECTOR: u16 = 1002;
pub const HC_PALETTE: u16 = 1003;
pub const HC_PROJECT: u16 = 1004;
pub const HC_MESSAGES: u16 = 1005;
pub const HC_APP_MODEL: u16 = 1006;
pub const HC_BUILD: u16 = 1007;
pub const HC_KEYS: u16 = 1008;
pub const HC_PROBLEMS: u16 = 1009;
