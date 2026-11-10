//! Редактор кода: окно с номерами строк, подсветкой синтаксиса,
//! пометками ошибок сборки и переходом к строке.
//!
//! Правка текста целиком делегируется `FileEditor` из tvision-rs
//! (клавиши WordStar, отмена, поиск и замена, буфер обмена, вопрос о
//! сохранении). Своя здесь только отрисовка: тот же текст, но в цвете.

use std::path::{Path, PathBuf};

use tvision::color::{attr, attr256, back, bg256, is_bg256, is_fg256, shade_bg, with_back, with_bg256, with_fg256, with_fore};
use tvision::widgets::editor::TAB_SIZE;
use tvision::*;

use crate::cmds::*;
use crate::highlight::{highlight_line, Class, Lang, State};
use crate::lsp::CompletionItem;
use crate::toolwins::MsgKind;

/// Атрибут `a` с фоном, взятым из `src` (цвет текста остаётся).
pub fn with_bg_of(a: Attr, src: Attr) -> Attr {
    if is_bg256(src) {
        with_bg256(a, bg256(src))
    } else {
        with_back(a, back(src))
    }
}

/// Ширина полосы номеров строк.
pub const GUTTER_W: i32 = 7;

/// Цвет класса лексемы на фоне обычного текста `normal`.
pub fn class_attr(normal: Attr, c: Class) -> Attr {
    // 256-цветная тема (gruvbox) — цвета gruvbox; иначе — 16 цветов BIOS.
    if is_bg256(normal) || is_fg256(normal) {
        let fg = match c {
            Class::Normal | Class::Punct => return normal,
            Class::Keyword => 167,
            Class::Type => 214,
            Class::Number => 175,
            Class::Str => 142,
            Class::Comment => 245,
            Class::Macro => 108,
            Class::Attr => 109,
            Class::Lifetime => 208,
        };
        with_fg256(normal, fg)
    } else {
        let fg = match c {
            Class::Normal => return normal,
            Class::Punct => return normal,
            Class::Keyword => 15,
            Class::Type => 11,
            Class::Number => 13,
            Class::Str => 10,
            Class::Comment => 7,
            Class::Macro => 12,
            Class::Attr => 3,
            Class::Lifetime => 13,
        };
        with_fore(normal, fg)
    }
}

// ======================================================================
// Редактор с подсветкой
// ======================================================================

/// Подчёркнутый диагностикой участок строки (столбцы — в символах).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Underline {
    pub line: usize,
    pub c0: usize,
    pub c1: usize,
    /// Ошибка (иначе — предупреждение).
    pub error: bool,
}

/// Открытый список дополнения.
///
/// Список рисует сам редактор поверх текста (рядом с курсором), а
/// клавиши Up/Down/PgUp/PgDn/Enter/Tab/Esc перехватывает, пока список
/// открыт. Всё остальное идёт в редактор как обычно, после чего список
/// фильтруется по набранному от `start` до курсора.
#[derive(Clone, Debug)]
pub struct Completion {
    pub items: Vec<CompletionItem>,
    /// С какого места заменять (начало слова или `textEdit.range.start`).
    pub start: Pos,
    /// Номера показанных вариантов (после фильтра).
    pub shown: Vec<usize>,
    pub sel: usize,
    pub top: usize,
}

/// Наибольшая высота списка дополнения.
pub const COMPLETION_ROWS: usize = 8;

/// Буква идентификатора Rust.
pub fn is_ident_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Совпадает ли `pat` с `text` как подпоследовательность (без учёта регистра).
fn subsequence(pat: &str, text: &str) -> bool {
    let mut t = text.chars().flat_map(|c| c.to_lowercase());
    pat.chars().flat_map(|c| c.to_lowercase()).all(|p| t.any(|c| c == p))
}

/// `FileEditor` с подсветкой синтаксиса, подчёркиванием диагностик
/// и списком дополнения.
pub struct CodeEditor {
    pub fe: FileEditor,
    pub lang: Lang,
    /// Диагностики сервера LSP для этого файла.
    pub diags: Vec<Underline>,
    /// Список дополнения (если открыт).
    pub completion: Option<Completion>,
    /// Подсвечивать строку курсора.
    pub hl_line: bool,
    /// Подсвечивать столбец курсора.
    pub hl_col: bool,
    /// Форматировать перед сохранением этой командой (`None` — не форматировать).
    pub fmt_cmd: Option<String>,
    /// Ошибка последнего форматирования (среда забирает её в окно сообщений).
    pub fmt_error: Option<String>,
}

impl CodeEditor {
    pub fn new(bounds: Rect, h: Option<ViewId>, v: Option<ViewId>, ind: Option<ViewId>, path: &Path) -> Self {
        let mut fe = FileEditor::new(bounds, h, v, ind, path);
        fe.editor.vs.help_ctx = HC_EDITOR;
        // Отступы в Rust — 4 пробела; табуляции в файле тоже показываются.
        CodeEditor {
            fe,
            lang: Lang::from_path(path),
            diags: Vec::new(),
            completion: None,
            hl_line: true,
            hl_col: true,
            fmt_cmd: None,
            fmt_error: None,
        }
    }

    /// Отформатировать текст rustfmt (если включено и это файл .rs).
    /// Курсор остаётся на той же строке; правка — один шаг отмены.
    /// `true` — текст изменился. При ошибке текст не трогается, а
    /// сообщение кладётся в `fmt_error`.
    pub fn format_now(&mut self) -> bool {
        let Some(cmd) = self.fmt_cmd.clone() else { return false };
        let path = self.fe.file_name.clone();
        if self.lang != Lang::Rust || path.extension().map(|x| x != "rs").unwrap_or(true) {
            return false;
        }
        let e = &mut self.fe.editor;
        let text = e.text();
        let new = match crate::rustfmt::format(&text, &path, &cmd) {
            Ok(t) => t,
            Err(err) => {
                self.fmt_error = Some(err);
                return false;
            }
        };
        // Редактор хранит строки без завершающего «\n» у последней:
        // сравниваем без учёта перевода строки в конце файла.
        if new.trim_end_matches('\n') == text.trim_end_matches('\n') {
            return false;
        }
        let cur = e.cursor();
        let delta = e.delta;
        let last = e.line_count().saturating_sub(1);
        let end = Pos::new(last, e.line(last).chars().count());
        e.set_select(Pos::new(0, 0), end);
        e.insert_text(&new, false);
        let line = cur.line.min(e.line_count().saturating_sub(1));
        let p = Pos::new(line, cur.col.min(e.line(line).chars().count()));
        e.set_select(p, p);
        e.scroll_to(delta.x, delta.y);
        e.track_cursor(false);
        self.completion = None;
        true
    }

    /// Начало слова под курсором (слева от курсора).
    pub fn word_start(&self) -> Pos {
        let cur = self.fe.editor.cursor();
        let line: Vec<char> = self.fe.editor.line(cur.line).chars().collect();
        let mut c = cur.col.min(line.len());
        while c > 0 && is_ident_char(line[c - 1]) {
            c -= 1;
        }
        Pos::new(cur.line, c)
    }

    /// Слово под курсором (целиком, в обе стороны) и его начало.
    pub fn word_at_cursor(&self) -> (String, Pos) {
        let cur = self.fe.editor.cursor();
        let line: Vec<char> = self.fe.editor.line(cur.line).chars().collect();
        let mut a = cur.col.min(line.len());
        while a > 0 && is_ident_char(line[a - 1]) {
            a -= 1;
        }
        let mut b = cur.col.min(line.len());
        while b < line.len() && is_ident_char(line[b]) {
            b += 1;
        }
        (line[a..b].iter().collect(), Pos::new(cur.line, a))
    }

    /// Открыть список дополнения.
    pub fn open_completion(&mut self, items: Vec<CompletionItem>, start: Pos) {
        if items.is_empty() {
            self.completion = None;
            return;
        }
        self.completion = Some(Completion { items, start, shown: Vec::new(), sel: 0, top: 0 });
        self.refilter();
    }

    /// Набранное от начала замены до курсора (`None` — курсор ушёл).
    fn typed_prefix(&self) -> Option<String> {
        let c = self.completion.as_ref()?;
        let cur = self.fe.editor.cursor();
        if cur.line != c.start.line || cur.col < c.start.col {
            return None;
        }
        let line: Vec<char> = self.fe.editor.line(cur.line).chars().collect();
        let s: String = line[c.start.col.min(line.len())..cur.col.min(line.len())].iter().collect();
        // Набран не идентификатор (пробел, скобка…) — список больше не нужен.
        if s.chars().any(|ch| !is_ident_char(ch)) {
            return None;
        }
        Some(s)
    }

    /// Отфильтровать варианты по набранному; закрыть список, если подходящих нет.
    pub fn refilter(&mut self) {
        let Some(prefix) = self.typed_prefix() else {
            self.completion = None;
            return;
        };
        let c = self.completion.as_mut().unwrap();
        let low = prefix.to_lowercase();
        // Сначала — начинающиеся с набранного, затем — содержащие его буквы по порядку.
        let mut shown: Vec<usize> = (0..c.items.len()).filter(|&i| c.items[i].filter.to_lowercase().starts_with(&low)).collect();
        shown.extend((0..c.items.len()).filter(|&i| !c.items[i].filter.to_lowercase().starts_with(&low) && subsequence(&low, &c.items[i].filter)));
        if shown.is_empty() {
            self.completion = None;
            return;
        }
        let keep = c.shown.get(c.sel).copied();
        c.shown = shown;
        c.sel = keep.and_then(|k| c.shown.iter().position(|&i| i == k)).unwrap_or(0);
        c.top = c.top.min(c.sel);
        if c.sel >= c.top + COMPLETION_ROWS {
            c.top = c.sel + 1 - COMPLETION_ROWS;
        }
    }

    /// Выбранный вариант.
    pub fn selected_completion(&self) -> Option<&CompletionItem> {
        let c = self.completion.as_ref()?;
        c.items.get(*c.shown.get(c.sel)?)
    }

    /// Вставить выбранный вариант (заменив набранное начало слова).
    pub fn accept_completion(&mut self) {
        let Some(item) = self.selected_completion().cloned() else { return };
        let start = self.completion.as_ref().unwrap().start;
        self.completion = None;
        let e = &mut self.fe.editor;
        let cur = e.cursor();
        // Если сервер предлагал заменить и хвост слова справа — заменяем
        // до конца слова, иначе только набранное.
        let line: Vec<char> = e.line(cur.line).chars().collect();
        let mut end = cur.col.min(line.len());
        if let Some(r) = item.range {
            if r.end.line == cur.line && r.end.col > end {
                while end < line.len() && is_ident_char(line[end]) {
                    end += 1;
                }
            }
        }
        e.set_select(start, Pos::new(cur.line, end));
        e.insert_text(&item.insert, false);
        // Курсор — внутрь скобок, если вставлен вызов без аргументов «f()»…
        // нет: оставляем после вставки, как делает большинство редакторов
        // без поддержки сниппетов.
        e.track_cursor(false);
    }

    fn move_completion(&mut self, delta: i32) {
        let Some(c) = &mut self.completion else { return };
        let n = c.shown.len() as i32;
        let s = (c.sel as i32 + delta).rem_euclid(n.max(1));
        // PgUp/PgDn не «заворачиваются» через край.
        let s = if delta.abs() > 1 { (c.sel as i32 + delta).clamp(0, n - 1) } else { s };
        c.sel = s as usize;
        if c.sel < c.top {
            c.top = c.sel;
        }
        if c.sel >= c.top + COMPLETION_ROWS {
            c.top = c.sel + 1 - COMPLETION_ROWS;
        }
    }

    /// Обновить полосы прокрутки и индикатор после правки в обход событий.
    fn resync(&mut self, ctx: &mut Ctx) {
        let mut nothing = Event::Broadcast(Message::new(0xFFFE));
        self.fe.handle_event(&mut nothing, ctx);
    }

    /// После набранного символа: `.` и `::` (Rust); в TOML — `.` (ключи
    /// `package.`, `dependencies.`) и открывающая `"` значения (`serde = "` —
    /// версии пакета от crates-lsp) сами открывают дополнение.
    fn after_typing(&mut self, key: Key, ctx: &mut Ctx) {
        let Key::Char(ch) = key else { return };
        let wanted = match self.lang {
            Lang::Rust => ch == '.' || ch == ':',
            Lang::Toml => ch == '.' || ch == '"',
            _ => false,
        };
        if !wanted {
            return;
        }
        let cur = self.fe.editor.cursor();
        let line: Vec<char> = self.fe.editor.line(cur.line).chars().collect();
        let before = |k: usize| if cur.col >= k { line.get(cur.col - k).copied() } else { None };
        if before(1) != Some(ch) {
            return;
        }
        let trigger = match ch {
            // «..» (диапазон) и число «1.» — не поле.
            '.' if before(2) == Some('.') || before(2).map(|c| c.is_ascii_digit()).unwrap_or(false) => return,
            '.' => ".",
            // Кавычка сразу после «=» (через пробелы) — начало значения.
            '"' => {
                let mut k = 2;
                while before(k) == Some(' ') {
                    k += 1;
                }
                if before(k) != Some('=') {
                    return;
                }
                "\""
            }
            _ if before(2) == Some(':') => "::",
            _ => return,
        };
        ctx.post_command(CM_COMPLETE, Info::Str(trigger.into()));
    }

    /// Строки списка дополнения: (строка экрана, столбец, текст, выбран ли).
    fn completion_rows(&self) -> Vec<(i32, i32, String, bool)> {
        let Some(c) = &self.completion else { return Vec::new() };
        let e = &self.fe.editor;
        let size = e.vs.size;
        let rows = c.shown.len().min(COMPLETION_ROWS);
        let label_w = c.shown.iter().map(|&i| c.items[i].label.chars().count()).max().unwrap_or(0);
        let detail_w = c.shown.iter().map(|&i| c.items[i].detail.chars().count()).max().unwrap_or(0).min(30);
        let w = (label_w + if detail_w > 0 { detail_w + 2 } else { 0 } + 4).clamp(16, 64).min(size.x.max(1) as usize) as i32;
        let cur_row = e.cursor().line as i32 - e.delta.y;
        let x0 = e.col_to_x(c.start.line, c.start.col) as i32 - e.delta.x - 2;
        let x = x0.clamp(0, (size.x - w).max(0));
        // Под курсором, если помещается; иначе — над ним.
        let y = if cur_row + 1 + rows as i32 <= size.y { cur_row + 1 } else { (cur_row - rows as i32).max(0) };
        let mut out = Vec::new();
        for r in 0..rows {
            let k = c.top + r;
            let Some(&i) = c.shown.get(k) else { break };
            let it = &c.items[i];
            let icon = match it.kind {
                2..=4 => 'ƒ',  // метод, функция, конструктор
                5 | 10 => '·', // поле, свойство
                6 => 'v',      // переменная
                7 | 22 => 'S', // класс, структура
                8 => 'T',      // интерфейс (типаж)
                9 => 'm',      // модуль
                13 => 'E',     // перечисление
                14 => 'k',     // ключевое слово
                15 => '»',     // сниппет
                20 => 'e',     // вариант перечисления
                21 => 'c',     // константа
                _ => ' ',
            };
            let mut text = format!(" {} {:<lw$}", icon, it.label, lw = label_w);
            if detail_w > 0 {
                let d: String = it.detail.chars().take(detail_w).collect();
                text.push_str("  ");
                text.push_str(&d);
            }
            let text: String = text.chars().take(w as usize).collect();
            let pad = w as usize - text.chars().count().min(w as usize);
            out.push((y + r as i32, x, format!("{}{}", text, " ".repeat(pad)), k == c.sel));
        }
        out
    }
}

impl View for CodeEditor {
    fn vs(&self) -> &ViewState {
        &self.fe.editor.vs
    }
    fn vs_mut(&mut self) -> &mut ViewState {
        &mut self.fe.editor.vs
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    fn into_any(self: Box<Self>) -> Box<dyn std::any::Any> {
        self
    }

    fn draw(&self, dc: &mut DrawCtx) {
        let ed = &self.fe.editor;
        let normal = dc.color(1);
        let selected = dc.color(2);
        let size = ed.vs.size;
        let w = size.x.max(0) as usize;
        let sel = ed.selection();
        let n = ed.line_count();
        let wide = is_bg256(normal) || is_fg256(normal);
        // Фон подчёркнутых диагностикой мест.
        let err_bg = |a: Attr| if wide { with_bg256(a, 52) } else { with_back(a, 4) };
        let warn_bg = |a: Attr| if wide { with_bg256(a, 58) } else { with_back(a, 6) };
        let popup = self.completion_rows();
        let (pop_n, pop_s) = if wide { (attr256(223, 237), attr256(235, 214)) } else { (attr(0, 7), attr(15, 2)) };
        // Строка и столбец курсора — чуть другим фоном (только в активном
        // редакторе, чтобы в остальных окнах не рябило).
        let active = ed.vs.is_focused();
        let cur = ed.cursor();
        let cur_bg = shade_bg(normal);
        let hl_line = active && self.hl_line;
        // Экранный столбец курсора (с учётом табуляций и широких символов).
        let cur_x = if active && self.hl_col { Some(ed.col_to_x(cur.line, cur.col)) } else { None };
        let top = ed.delta.y.max(0) as usize;
        let dx = ed.delta.x.max(0) as usize;
        // Состояние разбора на первой видимой строке.
        let mut state = State::Normal;
        if self.lang != Lang::Plain {
            for li in 0..top.min(n) {
                let l: Vec<char> = ed.line(li).chars().collect();
                highlight_line(&l, &mut state, self.lang);
            }
        }
        for row in 0..size.y {
            let mut b = DrawBuffer::new(w);
            let li = top + row as usize;
            let on_line = hl_line && li == cur.line;
            // Фон ячейки: строка или столбец курсора.
            let shade = |a: Attr, x: usize, cw: usize| {
                let on_col = cur_x.map(|cx| cx >= x && cx < x + cw).unwrap_or(false);
                if on_line || on_col {
                    with_bg_of(a, cur_bg)
                } else {
                    a
                }
            };
            b.move_char(0, ' ', if on_line { with_bg_of(normal, cur_bg) } else { normal }, w);
            if let Some(cx) = cur_x {
                if cx >= dx && cx - dx < w {
                    b.move_char(cx - dx, ' ', with_bg_of(normal, cur_bg), 1);
                }
            }
            if li < n {
                let line: Vec<char> = ed.line(li).chars().collect();
                let classes = highlight_line(&line, &mut state, self.lang);
                let in_sel = |col: usize| sel.map(|(a, bb)| Pos::new(li, col) >= a && Pos::new(li, col) < bb).unwrap_or(false);
                // Диагностика на символе: ошибка «главнее» предупреждения.
                let diag_at = |col: usize| -> Option<bool> {
                    let mut r = None;
                    for d in self.diags.iter().filter(|d| d.line == li) {
                        let c1 = if d.c1 > d.c0 { d.c1 } else { d.c0 + 1 };
                        if col >= d.c0 && col < c1 {
                            r = Some(r.unwrap_or(false) || d.error);
                        }
                    }
                    r
                };
                let mut x = 0usize;
                for (ci, &c) in line.iter().enumerate() {
                    let cw = if c == '\t' { TAB_SIZE - x % TAB_SIZE } else { tvision::text::char_width(c).max(1) };
                    let mut attr = if in_sel(ci) { selected } else { class_attr(normal, classes.get(ci).copied().unwrap_or(Class::Normal)) };
                    if !in_sel(ci) {
                        match diag_at(ci) {
                            Some(true) => attr = err_bg(attr),
                            Some(false) => attr = warn_bg(attr),
                            None => attr = shade(attr, x, cw),
                        }
                    }
                    let vis_a = x.max(dx);
                    let vis_b = (x + cw).min(dx + w);
                    if vis_a < vis_b {
                        if c == '\t' || vis_b - vis_a < cw {
                            b.move_char(vis_a - dx, ' ', attr, vis_b - vis_a);
                        } else {
                            let mut tmp = [0u8; 4];
                            b.move_str(x - dx, c.encode_utf8(&mut tmp), attr);
                        }
                    }
                    x += cw;
                    if x >= dx + w {
                        break;
                    }
                }
                if let Some((a, bb)) = sel {
                    let eol = Pos::new(li, line.len());
                    if eol >= a && eol < bb && x >= dx && x - dx < w {
                        b.move_char(x - dx, ' ', selected, 1);
                    }
                }
                // Диагностика «в конце строки» (например, нет `;`) — пробел с фоном.
                if x >= dx && x - dx < w {
                    match diag_at(line.len()) {
                        Some(true) => b.move_char(x - dx, ' ', err_bg(normal), 1),
                        Some(false) => b.move_char(x - dx, ' ', warn_bg(normal), 1),
                        None => {}
                    }
                }
            }
            for (py, px, text, on) in &popup {
                if *py == row {
                    b.move_str_limited(*px as usize, text, if *on { pop_s } else { pop_n }, w.saturating_sub(*px as usize));
                }
            }
            dc.write_line(0, row, size.x, 1, &b);
        }
    }

    fn palette(&self) -> &'static [u8] {
        self.fe.editor.palette
    }

    fn handle_event(&mut self, ev: &mut Event, ctx: &mut Ctx) {
        let focused = self.fe.editor.vs.is_focused();
        // Открытый список дополнения перехватывает клавиши выбора.
        if self.completion.is_some() {
            match ev {
                Event::KeyDown(k) if focused => {
                    let k = *k;
                    let page = COMPLETION_ROWS as i32 - 1;
                    match (k.key, k.mods) {
                        (Key::Up, Mods::NONE) => self.move_completion(-1),
                        (Key::Down, Mods::NONE) => self.move_completion(1),
                        (Key::PgUp, Mods::NONE) => self.move_completion(-page),
                        (Key::PgDn, Mods::NONE) => self.move_completion(page),
                        (Key::Enter, Mods::NONE) | (Key::Tab, Mods::NONE) => {
                            self.accept_completion();
                            self.resync(ctx);
                        }
                        (Key::Esc, _) => self.completion = None,
                        _ => {
                            // Остальное — в редактор, затем перефильтровать.
                            self.fe.handle_event(ev, ctx);
                            self.refilter();
                            self.after_typing(k.key, ctx);
                            return;
                        }
                    }
                    ev.clear();
                    return;
                }
                Event::MouseDown(_) => self.completion = None,
                _ => {}
            }
        }
        if let Event::KeyDown(k) = ev {
            // Ctrl+Space — дополнение (в терминалах приходит и как Ctrl+@ / NUL).
            if focused && ((k.key == Key::Char(' ') && k.mods.ctrl) || k.key == Key::Char('\0')) {
                ctx.post_command(CM_COMPLETE, Info::None);
                ev.clear();
                return;
            }
        }
        // Tab в коде — четыре пробела до ближайшей кратной позиции.
        if let Event::KeyDown(k) = ev {
            if k.key == Key::Tab && k.mods == Mods::NONE && self.fe.editor.vs.is_focused() && self.fe.editor.selection().is_none() {
                let col = self.fe.editor.cursor().col;
                let n = 4 - col % 4;
                self.fe.editor.insert_text(&" ".repeat(n), false);
                self.fe.editor.track_cursor(false);
                ev.clear();
                // Обновить полосы и индикатор — обработкой «пустого» события.
                self.resync(ctx);
                return;
            }
        }
        // Сохранение: сначала rustfmt.
        if focused && matches!(ev.command(), Some(cm::SAVE) | Some(cm::SAVE_AS)) && self.format_now() {
            self.resync(ctx);
        }
        let key = match ev {
            Event::KeyDown(k) if focused => Some(k.key),
            _ => None,
        };
        self.fe.handle_event(ev, ctx);
        if let Some(k) = key {
            self.after_typing(k, ctx);
        }
    }

    fn exec_result(&mut self, tag: u16, command: u16, view: Box<dyn View>, ctx: &mut Ctx) {
        self.fe.exec_result(tag, command, view, ctx);
    }

    fn update_commands(&self, ctx: &mut Ctx) {
        self.fe.update_commands(ctx);
    }

    fn valid(&mut self, command: u16, ctx: &mut Ctx) -> bool {
        // Закрытие с вопросом «сохранить?» — сохранится уже отформатированный текст.
        if matches!(command, cm::CLOSE | cm::QUIT) && self.fe.editor.modified && self.format_now() {
            self.resync(ctx);
        }
        self.fe.valid(command, ctx)
    }

    fn change_bounds(&mut self, bounds: Rect) {
        self.fe.change_bounds(bounds);
    }

    fn set_state(&mut self, flag: u16, enable: bool) {
        self.fe.set_state(flag, enable);
    }
}

// ======================================================================
// Номера строк
// ======================================================================

/// Полоса номеров строк слева от текста; помечает строки с ошибками.
pub struct Gutter {
    pub vs: ViewState,
    pub top: usize,
    pub count: usize,
    pub cur: usize,
    /// Пометки: (номер строки с 0, вид).
    pub marks: Vec<(usize, MsgKind)>,
}

impl Gutter {
    pub fn new(bounds: Rect) -> Self {
        let mut vs = ViewState::new(bounds);
        vs.grow_mode = gf::GROW_HI_Y;
        Gutter { vs, top: 0, count: 1, cur: 0, marks: Vec::new() }
    }
}

impl View for Gutter {
    tvision::impl_view_basics!();

    fn draw(&self, dc: &mut DrawCtx) {
        let base = dc.color(6);
        let wide = is_bg256(base) || is_fg256(base);
        let dim = if wide { with_fg256(base, 243) } else { with_fore(base, 8) };
        let cur = if wide { with_fg256(base, 214) } else { with_fore(base, 14) };
        let err = if wide { with_fg256(base, 167) } else { with_fore(base, 12) };
        let warn = if wide { with_fg256(base, 214) } else { with_fore(base, 14) };
        let w = self.vs.size.x.max(1) as usize;
        for row in 0..self.vs.size.y {
            let li = self.top + row as usize;
            let mut b = DrawBuffer::new(w);
            b.move_char(0, ' ', dim, w);
            if li < self.count {
                let s = format!("{:>width$}", li + 1, width = w.saturating_sub(2));
                b.move_str(1, &s, if li == self.cur { cur } else { dim });
                if let Some((_, k)) = self.marks.iter().find(|(l, _)| *l == li) {
                    match k {
                        MsgKind::Error => b.move_str(0, "●", err),
                        _ => b.move_str(0, "▲", warn),
                    };
                }
            }
            b.move_char(w - 1, '│', dim, 1);
            dc.write_line(0, row, self.vs.size.x, 1, &b);
        }
    }
}

// ======================================================================
// Окно редактора кода
// ======================================================================

/// Окно редактора кода.
pub struct CodeWindow {
    pub win: Window,
    pub editor_id: ViewId,
    pub gutter_id: ViewId,
    /// Каталог проекта — заголовок окна показывается относительно него.
    pub base: Option<PathBuf>,
}

impl CodeWindow {
    pub fn new(bounds: Rect, path: &Path, number: i16, base: Option<PathBuf>) -> Self {
        let mut win = Window::new(bounds, "", number);
        win.group.vs.options |= of::TILEABLE;
        let size = win.group.vs.size;
        let hsb = win.add(ScrollBar::new(Rect::new(18, size.y - 1, size.x - 2, size.y)));
        let vsb = win.standard_scroll_bar(true);
        let ind = win.add(Indicator::new(Rect::new(2, size.y - 1, 16, size.y)));
        let gutter_id = win.add(Gutter::new(Rect::new(1, 1, 1 + GUTTER_W, size.y - 1)));
        let ed = CodeEditor::new(Rect::new(1 + GUTTER_W, 1, size.x - 1, size.y - 1), Some(hsb), Some(vsb), Some(ind), path);
        let editor_id = win.add(ed);
        let mut w = CodeWindow { win, editor_id, gutter_id, base };
        w.sync();
        w
    }

    pub fn editor(&self) -> &CodeEditor {
        self.win.group.find::<CodeEditor>(self.editor_id).unwrap()
    }

    pub fn editor_mut(&mut self) -> &mut CodeEditor {
        let id = self.editor_id;
        self.win.group.find_mut::<CodeEditor>(id).unwrap()
    }

    /// Путь файла (пустой — безымянный).
    pub fn path(&self) -> PathBuf {
        self.editor().fe.file_name.clone()
    }

    /// Перейти к строке и столбцу (нумерация с 1).
    pub fn goto(&mut self, line: usize, col: usize) {
        let e = &mut self.editor_mut().fe.editor;
        let p = Pos::new(line.saturating_sub(1), col.saturating_sub(1));
        e.set_select(p, p);
        e.track_cursor(true);
        self.sync();
    }

    /// Пометки ошибок и предупреждений.
    pub fn set_marks(&mut self, marks: Vec<(usize, MsgKind)>) {
        let id = self.gutter_id;
        if let Some(g) = self.win.group.find_mut::<Gutter>(id) {
            g.marks = marks;
        }
    }

    /// Обновить полосу номеров и заголовок.
    fn sync(&mut self) {
        let (top, count, cur, title) = {
            let ce = self.editor();
            let e = &ce.fe.editor;
            let path = &ce.fe.file_name;
            let title = if path.as_os_str().is_empty() {
                "Без имени".to_string()
            } else {
                match &self.base {
                    Some(b) => path.strip_prefix(b).map(|p| p.display().to_string()).unwrap_or_else(|_| path.display().to_string()),
                    None => path.display().to_string(),
                }
            };
            (e.delta.y.max(0) as usize, e.line_count(), e.cursor().line, title)
        };
        let gid = self.gutter_id;
        if let Some(g) = self.win.group.find_mut::<Gutter>(gid) {
            g.top = top;
            g.count = count;
            g.cur = cur;
        }
        if self.win.title != title {
            self.win.title = title;
        }
    }
}

impl View for CodeWindow {
    tvision::impl_group_basics!(win.group);

    fn as_window(&self) -> Option<&Window> {
        Some(&self.win)
    }

    fn draw(&self, dc: &mut DrawCtx) {
        self.win.draw_window(dc);
    }

    fn palette(&self) -> &'static [u8] {
        self.win.palette
    }

    fn set_state(&mut self, flag: u16, enable: bool) {
        self.win.set_window_state(flag, enable);
    }

    fn handle_event(&mut self, ev: &mut Event, ctx: &mut Ctx) {
        self.win.handle_window_event(ev, ctx);
        self.sync();
    }

    fn valid(&mut self, command: u16, ctx: &mut Ctx) -> bool {
        self.win.group.valid(command, ctx)
    }

    fn size_limits(&self, owner_size: Point) -> (Point, Point) {
        (Point::new(24, 6), owner_size)
    }
}
