//! Отслеживание указателя и подготовка строк всплывающей подсказки редактора.
//!
//! Общий путь данных:
//! 1. Обёртка [HoverBackend] наблюдает сырые события терминала до их обработки
//!    интерфейсом, обновляет [Pointer] и поглощает колесо внутри подсказки.
//! 2. [hit] находит верхний видимый редактор под указателем, а [symbol_at]
//!    переводит координаты его ячейки в позицию символа, подходящего для запроса.
//! 3. Код связи с LSP в ide/lsp_glue.rs выдерживает задержку 400 мс, отправляет
//!    запрос, проверяет актуальность ответа и создаёт либо удаляет [Popup].
//!    Обмен с сервером реализован вне модуля; здесь хранится задержка скрытия.
//! 4. Редактор вызывает [Popup::rows] и рисует полученные строки поверх текста.
//!    Это данные для отрисовки, а не отдельное окно: Popup не получает фокус.
//!
//! Смена поколения указателя позволяет сразу прекратить отрисовку старой
//! подсказки. Скрытие над пустым местом, проверка изменений документа и выбор
//! между подсказкой и дополнением дополнительно выполняются вызывающим кодом.
//! Координаты здесь измеряются в ячейках терминала; позиции текста — в строках
//! и Unicode-символах с нуля, а не в байтах или единицах кодировки LSP.
use crate::codeedit::{CodeEditor, Underline};
use crate::lsp::{col_from_lsp, Diagnostic as LspDiagnostic, Encoding, Severity};
#[path = "hover_markdown.rs"]
mod markdown;
pub use markdown::Style;
use std::{
    cell::RefCell,
    rc::Rc,
    time::{Duration, Instant},
};
use tvision::{
    platform::{Backend, RawEvent},
    *,
};

/// Состояние ввода, общее для терминальной обёртки, среды и подсказки.
///
/// Хранит позицию, поколение и область уже открытой подсказки.
/// Семантику имени и запросы LSP определяет среда.
#[derive(Clone, Debug)]
pub struct Pointer {
    /// Позиция на экране в ячейках; None означает сброс наведения после ввода,
    /// изменения размера или приостановки терминала, а не обязательно уход мыши.
    pub pos: Option<Point>,
    /// Номер состояния ввода. Подсказка запоминает его и перестаёт рисоваться,
    /// когда текущее поколение отличается от сохранённого. Движение внутри
    /// активной области и переход к ней в пределах задержки сохраняют поколение.
    pub generation: u64,
    /// Время последнего обновления в read (либо создания состояния).
    /// Среда использует его для задержки; suspend это поле не обновляет.
    pub since: Instant,
    /// Область показанной подсказки и состояние её прокрутки.
    pub active: Option<Active>,
}
impl Default for Pointer {
    /// Создаёт состояние без позиции, с поколением 0 и текущим временем.
    /// Не читает терминал и не запускает запросы; только формирует значение.
    fn default() -> Self {
        Self { pos: None, generation: 0, since: Instant::now(), active: None }
    }
}
/// Состояние интерактивной области в экранных координатах, без захвата фокуса.
#[derive(Clone, Debug)]
pub struct Active {
    /// Поколение запроса, которому принадлежит область.
    pub generation: u64,
    /// Начало редактора на экране для перевода локальных координат рамки.
    pub origin: Point,
    /// Видимые ячейки исходного имени либо строки диагностируемого участка.
    pub source: Vec<Rect>,
    /// Экранная рамка подсказки, включая её границы.
    pub bounds: Rect,
    /// Индекс первой видимой строки текста.
    pub offset: usize,
    /// Наибольший индекс, при котором видна последняя строка.
    pub max_offset: usize,
    /// Начало перехода через промежуток между именем и рамкой.
    pub leaving: Option<Instant>,
}
impl Pointer {
    /// Завершает задержку скрытия. Вызывается и без новых событий мыши.
    pub fn expire(&mut self) {
        if self
            .active
            .as_ref()
            .is_some_and(|a| a.generation != self.generation || a.leaving.is_some_and(|t| t.elapsed() >= Duration::from_millis(300)))
        {
            self.active = None;
            self.generation = self.generation.wrapping_add(1);
        }
    }
    /// Обновляет положение; возвращает true, пока открытая подсказка удерживает наведение.
    fn motion(&mut self, pos: Point) -> bool {
        self.expire();
        if self.pos != Some(pos) {
            self.since = Instant::now();
        }
        self.pos = Some(pos);
        if let Some(a) = &mut self.active {
            if a.bounds.contains(pos) {
                a.leaving = None;
                return true;
            }
            if a.source.iter().any(|r| r.contains(pos)) {
                a.leaving = None;
            } else {
                // Backend не предоставляет перенос системного указателя. Даём
                // 300 мс на самостоятельный переход от имени к подсказке.
                a.leaving.get_or_insert_with(Instant::now);
            }
            return true;
        }
        false
    }
}

/// Полученная от LSP диагностика с теми же диапазонами, что рисует редактор.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    /// Участки строк в Unicode-символах; пустой диапазон рисуется одной ячейкой.
    pub spans: Vec<Underline>,
    /// Полный текст с уровнем, источником и кодом; всегда plaintext.
    pub text: String,
    pub severity: Severity,
}
impl Diagnostic {
    /// Преобразует координаты LSP в координаты редактора. Конец диапазона
    /// исключён, поэтому end=(следующая строка, 0) не отмечает следующую строку.
    pub fn new(ed: &CodeEditor, d: &LspDiagnostic, enc: Encoding) -> Self {
        let e = &ed.fe.editor;
        let mut spans = Vec::new();
        if d.range.end >= d.range.start {
            for line in d.range.start.line..=d.range.end.line.min(e.line_count().saturating_sub(1)) {
                if line == d.range.end.line && d.range.end.col == 0 && line > d.range.start.line {
                    break;
                }
                let text = e.line(line);
                let c0 = if line == d.range.start.line { col_from_lsp(&text, d.range.start.col, enc) } else { 0 };
                let c1 = if line == d.range.end.line { col_from_lsp(&text, d.range.end.col, enc) } else { text.chars().count() };
                spans.push(Underline { line, c0, c1: c1.max(c0), error: d.severity == Severity::Error });
            }
        }
        let level = match d.severity {
            Severity::Error => "Ошибка",
            Severity::Warning => "Предупреждение",
            Severity::Info => "Информация",
            Severity::Hint => "Подсказка",
        };
        let details = [d.source.as_str(), d.code.as_str()].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" · ");
        let header = if details.is_empty() { level.to_owned() } else { format!("{level} ({details})") };
        Self { spans, text: format!("{header}\n{}", d.message), severity: d.severity }
    }
    /// Проверяет ячейку текста по правилам подсветки, включая пустой диапазон.
    fn contains(&self, at: Pos) -> bool {
        self.spans.iter().any(|s| s.line == at.line && at.col >= s.c0 && at.col < s.c1.max(s.c0 + 1))
    }
}
/// Все сообщения в позиции, стабильно упорядоченные по серьёзности.
pub fn diagnostics_at(ed: &CodeEditor, at: Pos) -> Vec<&Diagnostic> {
    let mut found: Vec<_> = ed.hover_diags.iter().filter(|d| d.contains(at)).collect();
    found.sort_by_key(|d| d.severity);
    found
}
/// Формирует только буквальную часть подсказки, не смешивая её с Markdown Hover.
pub fn diagnostic_text(ed: &CodeEditor, at: Pos) -> String {
    diagnostics_at(ed, at).iter().map(|d| d.text.as_str()).collect::<Vec<_>>().join("\n\n")
}
/// Регистрирует имя либо все строки диагностики. При дополнении ответа Hover
/// сохраняет экранное начало редактора: мышь уже может находиться внутри рамки.
pub fn activate(pointer: &SharedPointer, ed: &CodeEditor, at: Pos, local: Point) {
    let mut p = pointer.borrow_mut();
    let Some(screen) = p.pos else { return };
    let previous = p.active.take().filter(|a| a.generation == p.generation);
    let origin = previous.as_ref().map(|a| a.origin).unwrap_or(screen - local);
    let e = &ed.fe.editor;
    let mut source = Vec::new();
    let mut add_span = |line: usize, c0: usize, c1: usize| {
        let y = line as i32 - e.delta.y;
        if y < 0 || y >= e.vs.size.y {
            return;
        }
        let x0 = e.col_to_x(line, c0) as i32 - e.delta.x;
        let x1 = (e.col_to_x(line, c1) as i32 - e.delta.x).max(x0 + 1);
        if x1 > 0 && x0 < e.vs.size.x {
            source.push(Rect::new(origin.x + x0.max(0), origin.y + y, origin.x + x1.min(e.vs.size.x), origin.y + y + 1));
        }
    };
    let found = diagnostics_at(ed, at);
    if found.is_empty() {
        let chars: Vec<char> = e.line(at.line).chars().collect();
        let is_name = |c: char| c.is_alphanumeric() || c == '_';
        let mut start = at.col.min(chars.len());
        let mut end = start;
        if chars.get(start).is_some_and(|c| is_name(*c)) {
            while start > 0 && is_name(chars[start - 1]) {
                start -= 1;
            }
            while end < chars.len() && is_name(chars[end]) {
                end += 1;
            }
        }
        add_span(at.line, start, end.max(start + 1).min(chars.len()));
    } else {
        for d in found {
            for s in &d.spans {
                add_span(s.line, s.c0, s.c1.max(s.c0 + 1));
            }
        }
    }
    let mut active = previous.unwrap_or(Active {
        generation: p.generation,
        origin,
        source: vec![],
        bounds: Rect::new(0, 0, 0, 0),
        offset: 0,
        max_offset: 0,
        leaving: None,
    });
    active.source = source;
    p.active = Some(active);
    p.motion(screen);
}
/// Совместно используемое состояние в одном потоке интерфейса.
/// Rc сохраняет один объект у нескольких владельцев, RefCell позволяет
/// обновлять его через общую ссылку с проверкой заимствований во время работы.
pub type SharedPointer = Rc<RefCell<Pointer>>;
/// Декоратор терминального Backend: наблюдает ввод для Hover и делегирует
/// остальные операции исходному терминалу, сохраняя его возможности.
pub struct HoverBackend {
    /// Исходный терминал; обёртка владеет им и перенаправляет вызовы.
    inner: Box<dyn Backend>,
    /// Тот же объект состояния, который читают среда и уже созданный Popup.
    pointer: SharedPointer,
}
impl HoverBackend {
    /// Принимает терминал inner и общую ссылку pointer, возвращает обёртку.
    /// Не читает события и не сбрасывает переданное состояние указателя.
    pub fn new(inner: Box<dyn Backend>, pointer: SharedPointer) -> Self {
        Self { inner, pointer }
    }
}
impl Backend for HoverBackend {
    /// Возвращает текущие ширину и высоту исходного терминала в ячейках.
    /// Состояние наведения не изменяет.
    fn size(&self) -> Point {
        self.inner.size()
    }
    /// Передаёт timeout исходному терминалу как предел ожидания события.
    /// Возвращает исходное событие либо None при отсутствии или поглощении колеса.
    ///
    /// Дополнительно обновляет позицию, поколение и время в pointer.
    /// При None состояние сохраняется: простой сам по себе не отменяет Hover.
    fn read(&mut self, timeout: Duration) -> Option<RawEvent> {
        let ev = self.inner.read(timeout);
        let mut p = self.pointer.borrow_mut();
        p.expire();
        if let Some(RawEvent::Mouse { pos, buttons: 0, wheel, .. }) = &ev {
            let previous = p.pos;
            let held = p.motion(*pos);
            if *wheel != 0 {
                if let Some(a) = &mut p.active {
                    if a.bounds.contains(*pos) {
                        // Поглощаем колесо до диспетчеризации: редактор не прокручивается.
                        a.offset = (a.offset as isize + *wheel as isize * 3).clamp(0, a.max_offset as isize) as usize;
                        return None;
                    }
                }
            } else if held {
                return ev;
            } else if previous == Some(*pos) {
                return ev;
            }
        }
        if let Some(event) = &ev {
            p.active = None;
            p.pos = match event {
                RawEvent::Mouse { pos, buttons: 0, wheel: 0, .. } => Some(*pos),
                _ => None,
            };
            p.generation = p.generation.wrapping_add(1);
            p.since = Instant::now();
        }
        ev
    }
    /// Передаёт кадр s, позицию видимого курсора c (None — скрыт) и флаг
    /// режима вставки i исходному терминалу. Вывод выполняет inner;
    /// метод ничего не возвращает и состояние Hover не меняет.
    fn flush(&mut self, s: &ScreenBuffer, c: Option<Point>, i: bool) {
        self.inner.flush(s, c, i);
    }
    /// Сбрасывает позицию и меняет поколение перед приостановкой терминала.
    /// Делегирует переход из полноэкранного режима inner; значение не возвращает.
    /// Время since сохраняется, но без позиции новое наведение не запрашивается.
    fn suspend(&mut self) {
        let mut p = self.pointer.borrow_mut();
        p.pos = None;
        p.active = None;
        p.generation = p.generation.wrapping_add(1);
        self.inner.suspend();
    }
    /// Возобновляет исходный терминал, не восстанавливая старое наведение.
    /// Ничего не возвращает; новая позиция появится после подходящего события.
    fn resume(&mut self) {
        self.inner.resume();
    }
    /// Возвращает признак подключения системного буфера исходного терминала.
    /// Сам буфер не читает и не изменяет; состояние указателя сохраняет.
    fn system_clipboard(&self) -> bool {
        self.inner.system_clipboard()
    }
    /// Возвращает заимствованный последний кадр inner, если он его хранит.
    /// Обычно нужен тестовому терминалу; нового кадра не создаёт.
    fn last_frame(&self) -> Option<&ScreenBuffer> {
        self.inner.last_frame()
    }
}

/// Ищет редактор и имя либо маркер диагностики под point в координатах group.
///
/// Для корневой группы передают экранную позицию; при рекурсии координаты
/// переводятся в содержимое дочерней группы. Результат — идентификатор
/// редактора, текстовая позиция и точка относительно самого редактора.
/// None возвращается при отсутствии подходящего видимого представления,
/// редактора, имени или диагностики. Фокус и содержимое представлений не изменяются.
pub fn hit(group: &Group, point: Point) -> Option<(ViewId, Pos, Point)> {
    // Последние дети рисуются поверх предыдущих. Проверяем только верхнего:
    // непрозрачная для поиска панель не должна открывать подсказку за собой.
    let child = group.children.iter().rev().find(|v| v.vs().is_visible() && v.vs().bounds().contains(point))?;
    // Origin задан относительно родителя: на каждом уровне вычитаем ровно
    // одно смещение, не смешивая экранные координаты с координатами текста.
    let local = point - child.vs().origin;
    if let Some(ed) = child.as_any().downcast_ref::<CodeEditor>() {
        return position_at(ed, local)
            .filter(|at| symbol_at(ed, local).is_some() || !diagnostics_at(ed, *at).is_empty())
            .map(|at| (ed.vs().id, at, local));
    }
    hit(child.as_group()?, local)
}

/// Переводит ячейку local внутри видимой области ed в позицию символа.
///
/// Возвращает Pos с индексами строки и символа с нуля, учитывая прокрутку.
/// За границами редактора, после конца строки, на пустой строке, пробеле
/// или пунктуации возвращает None. Редактор и курсор не изменяет.
///
/// Это предварительный фильтр букв, цифр и подчёркивания, а не разбор языка:
/// он не выделяет имя целиком и не отличает комментарий от настоящего кода.
/// Семантические сведения о выбранной позиции должен предоставить LSP.
pub fn symbol_at(ed: &CodeEditor, local: Point) -> Option<Pos> {
    let at = position_at(ed, local)?;
    ed.fe.editor.line(at.line).chars().nth(at.col).filter(|ch| ch.is_alphanumeric() || *ch == '_').map(|_| at)
}
/// Позиция любой ячейки текста, включая пробел, табуляцию и ровно одну
/// ячейку за концом строки для видимого маркера пустого диапазона.
pub fn position_at(ed: &CodeEditor, local: Point) -> Option<Pos> {
    let e = &ed.fe.editor;
    if local.x < 0 || local.y < 0 || local.x >= e.vs.size.x || local.y >= e.vs.size.y {
        return None;
    }
    // delta хранит смещение прокрутки: y — строки, x — экранные ячейки.
    // Складываем смещения до поиска символа; модель редактора держит их
    // неотрицательными, а local выше проверена на попадание в видимую область.
    let line = (local.y + e.delta.y) as usize;
    if line >= e.line_count() {
        return None;
    }
    let x = (local.x + e.delta.x) as usize;
    let mut left = 0;
    // chars() даёт индекс Unicode-символа, а left считает экранные ячейки.
    // Это разные величины для табуляции и широких символов, например иероглифов.
    for (col, ch) in e.line(line).chars().enumerate() {
        // Табуляция идёт до следующей позиции TAB_SIZE от начала строки,
        // а не от левого края экрана. Минимум одна ячейка согласован
        // с отрисовкой редактора; полноценный разбор графем здесь не выполняется.
        let width = if ch == '\t' {
            tvision::widgets::editor::TAB_SIZE - left % tvision::widgets::editor::TAB_SIZE
        } else {
            tvision::text::char_width(ch).max(1)
        };
        // Попадание в любую ячейку широкого символа возвращает один и тот же
        // индекс. Пробелы и табуляция сохраняют собственную позицию диагностики.
        if x < left + width {
            return Some(Pos::new(line, col));
        }
        left += width;
    }
    (x == left).then(|| Pos::new(line, e.line(line).chars().count()))
}

/// Данные подсказки для отрисовки поверх текста, без собственного View.
/// Создание и удаление объекта, а также актуальность ответа LSP контролирует
/// среда; здесь дополнительно проверяется поколение ввода перед рисованием.
pub struct Popup {
    /// Общая ссылка на текущее состояние ввода для проверки актуальности.
    pub pointer: SharedPointer,
    /// Поколение, для которого получен текст; должно совпадать с pointer.
    pub generation: u64,
    /// Точка наведения в ячейках относительно видимой области редактора.
    pub at: Point,
    /// Текст ответа, уже извлечённый из JSON в модуле lsp.
    /// Может содержать переводы строк и разметку Markdown.
    pub text: String,
    /// Только ответы Markdown проходят через парсер; plaintext сохраняет разметку.
    pub markdown: bool,
    /// Диагностики всегда выводятся буквально перед ответом Hover.
    pub diagnostics: String,
}
impl Popup {
    /// Готовит строки рамки и содержимого для редактора размером size.
    ///
    /// Каждый кортеж содержит (y, x, текст, стили Unicode-символов). Координаты
    /// локальны редактору; отсутствующий стиль означает обычный текст. Метод обновляет область попадания и предел прокрутки:
    /// сам не рисует, не получает фокус и не удаляет Popup.
    ///
    /// Пустой вектор означает устаревшее поколение, слишком маленькую область
    /// либо отсутствие строк после фильтрации. Ширина содержимого ограничена
    /// 76 ячейками, высота — 12 строками и доступным местом внутри рамки.
    pub fn rows(&self, size: Point) -> Vec<(i32, i32, String, Vec<Style>)> {
        if self.pointer.borrow().generation != self.generation || size.x < 6 || size.y < 3 {
            return Vec::new();
        }
        let width = (size.x - 2).min(76) as usize;
        let mut lines = markdown::render(&self.diagnostics, false, width);
        let hover = markdown::render(&self.text, self.markdown, width);
        if !lines.is_empty() && !hover.is_empty() {
            lines.push(vec![]);
        }
        lines.extend(hover);
        let limit = (size.y - 2).min(12) as usize;
        let max_offset = lines.len().saturating_sub(limit);
        if lines.is_empty() {
            return Vec::new();
        }
        let visible = lines.len().min(limit);
        let height = visible as i32 + 2;
        let x = self.at.x.min(size.x - width as i32 - 2).max(0);
        let y = if self.at.y + height < size.y { self.at.y + 1 } else { (self.at.y - height).max(0) };
        let mut offset = 0;
        if let Some(a) = &mut self.pointer.borrow_mut().active {
            if a.generation == self.generation {
                a.max_offset = max_offset;
                a.offset = a.offset.min(max_offset);
                offset = a.offset;
                a.bounds = Rect::new(a.origin.x + x, a.origin.y + y, a.origin.x + x + width as i32 + 2, a.origin.y + y + height);
            }
        }
        // Ползунок занимает правую рамку: текст не теряет колонку при прокрутке.
        let thumb_len = (visible * visible / lines.len()).max(1);
        let thumb_at = if max_offset == 0 { 0 } else { offset * (visible - thumb_len) / max_offset };
        let mut result = vec![(y, x, format!("┌{}┐", "─".repeat(width)), vec![])];
        for (i, line) in lines.iter().skip(offset).take(limit).enumerate() {
            let used: usize = line.iter().map(|(c, _)| tvision::text::char_width(*c).max(1)).sum();
            let edge = if max_offset == 0 {
                '│'
            } else if (thumb_at..thumb_at + thumb_len).contains(&i) {
                '█'
            } else {
                '░'
            };
            let text: String = line.iter().map(|(c, _)| *c).collect();
            let mut styles = vec![Style::default()];
            styles.extend(line.iter().map(|(_, style)| *style));
            result.push((y + 1 + i as i32, x, format!("│{}{}{edge}", text, " ".repeat(width.saturating_sub(used))), styles));
        }
        result.push((y + height - 1, x, format!("└{}┘", "─".repeat(width)), vec![]));
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{json, lsp::parse_hover};
    use tvision::platform::HeadlessBackend;

    /// Проверяет входные форматы текста для Popup: MarkupContent, массив
    /// MarkedString, строку и пустые ответы. Вызывает парсер из модуля lsp,
    /// а не реализует разбор JSON в модуле наведения.
    #[test]
    fn hover_contents_variants() {
        for (input, expected) in [
            (r#"{"contents":{"kind":"markdown","value":"fn foo()\nDocs"}}"#, "fn foo()\nDocs"),
            (r#"{"contents":[{"language":"rust","value":"fn foo()"},"Docs"]}"#, "```\nfn foo()\n```\n\nDocs"),
            (r#"{"contents":"Docs"}"#, "Docs"),
            ("null", ""),
            (r#"{"contents":[]}"#, ""),
        ] {
            assert_eq!(parse_hover(&json::parse(input).unwrap()).0, expected);
        }
    }

    /// Формат MarkupContent обязателен к сохранению; legacy-строки — Markdown,
    /// а language/value — буквальный код, включая обратные кавычки внутри него.
    #[test]
    fn hover_preserves_format_and_legacy_code() {
        let (text, md) = parse_hover(&json::parse(r#"{"contents":{"kind":"plaintext","value":"**literal**\n```"}}"#).unwrap());
        assert!(!md);
        assert_eq!(text, "**literal**\n```");
        let (text, md) = parse_hover(&json::parse(r#"{"contents":{"language":"rust","value":"let s = \"```\";"}}"#).unwrap());
        assert!(md);
        let rows = markdown::render(&text, md, 60);
        let visible: String = rows.iter().flatten().map(|(c, _)| c).collect();
        assert_eq!(visible, "let s = \"```\";");
        assert!(parse_hover(&json::parse(r#"{"contents":"**legacy**"}"#).unwrap()).1);
    }

    /// Проверяет попадание в табуляцию, вторую ячейку широкого символа, пробел,
    /// конец и пустые строки, затем перевод координат при прокрутке.
    /// Изменяет только созданный тестом редактор.
    #[test]
    fn hover_symbol_hit_test_handles_tabs_unicode_and_scroll() {
        let mut ed = CodeEditor::new(Rect::new(0, 0, 30, 8), None, None, None, std::path::Path::new(""));
        ed.fe.editor.insert_text("\t变量 foo\n\nbar", false);
        let tab = tvision::widgets::editor::TAB_SIZE as i32;
        assert_eq!(symbol_at(&ed, Point::new(0, 0)), None);
        assert_eq!(symbol_at(&ed, Point::new(tab + 1, 0)), Some(Pos::new(0, 1)));
        assert_eq!(symbol_at(&ed, Point::new(tab + 4, 0)), None);
        assert_eq!(symbol_at(&ed, Point::new(25, 0)), None);
        assert_eq!(symbol_at(&ed, Point::new(0, 1)), None);
        assert_eq!(symbol_at(&ed, Point::new(0, 5)), None);
        ed.fe.editor.delta = Point::new(tab + 5, 0);
        assert_eq!(symbol_at(&ed, Point::new(0, 0)), Some(Pos::new(0, 4)));
        ed.fe.editor.delta = Point::new(0, 2);
        assert_eq!(symbol_at(&ed, Point::new(1, 0)), Some(Pos::new(2, 1)));
    }

    /// Проверяет рамку у правого нижнего края малого редактора, длинный текст
    /// с широкими символами и отсутствие управляющих символов в результате.
    /// Затем подаёт клавишу через HeadlessBackend: старый Popup должен
    /// перестать выдавать строки после смены поколения.
    #[test]
    fn hover_popup_fits_and_hides_on_input() {
        let shared = SharedPointer::default();
        let popup = Popup {
            pointer: shared.clone(),
            generation: 0,
            at: Point::new(19, 5),
            markdown: false,
            diagnostics: String::new(),
            text: format!("\x1b[31m{}\n{}", "变量".repeat(30), "docs\n".repeat(30)),
        };
        let rows = popup.rows(Point::new(20, 6));
        assert!(!rows.is_empty());
        for (y, x, line, _) in rows {
            assert!((0..6).contains(&y));
            assert!(x >= 0);
            assert!(!line.chars().any(char::is_control));
            assert!(x as usize + line.chars().map(|c| tvision::text::char_width(c).max(1)).sum::<usize>() <= 20);
        }
        let mut backend = HeadlessBackend::new(20, 6);
        backend.push_key(KeyEvent::new(Key::Char('a'), Mods::NONE));
        let mut backend = HoverBackend::new(Box::new(backend), shared);
        assert!(matches!(backend.read(Duration::ZERO), Some(RawEvent::Key(_))));
        assert!(popup.rows(Point::new(20, 6)).is_empty());
    }

    /// Проверяет передачу сырых событий без подмены: первое движение меняет
    /// позицию, повтор той же точки сохраняет поколение, а Resize сбрасывает
    /// наведение. Используется только очередь тестового терминала.
    #[test]
    fn hover_backend_tracks_motion_without_changing_events() {
        let pointer = SharedPointer::default();
        let event = RawEvent::Mouse { pos: Point::new(8, 9), buttons: 0, mods: Mods::NONE, wheel: 0 };
        let mut backend = HeadlessBackend::new(20, 20);
        backend.input.push_back(event.clone());
        backend.input.push_back(event.clone());
        backend.input.push_back(RawEvent::Resize(Point::new(40, 20)));
        let mut backend = HoverBackend::new(Box::new(backend), pointer.clone());
        assert_eq!(backend.read(Duration::ZERO), Some(event.clone()));
        let generation = pointer.borrow().generation;
        assert_eq!(pointer.borrow().pos, Some(Point::new(8, 9)));
        assert_eq!(backend.read(Duration::ZERO), Some(event));
        assert_eq!(pointer.borrow().generation, generation);
        backend.read(Duration::ZERO);
        assert!(pointer.borrow().pos.is_none());
        assert_ne!(pointer.borrow().generation, generation);
    }
    /// Длинный текст сохраняется целиком: колесо прокручивает строки только
    /// подсказки, крайние положения ограничены, выход запускает задержку скрытия.
    #[test]
    fn scroll_is_consumed_and_long_popup_delays_exit() {
        let pointer = SharedPointer::default();
        let mut ed = CodeEditor::new(Rect::new(0, 0, 30, 8), None, None, None, std::path::Path::new(""));
        ed.fe.editor.insert_text("helper", false);
        pointer.borrow_mut().pos = Some(Point::new(2, 0));
        activate(&pointer, &ed, Pos::new(0, 2), Point::new(2, 0));
        let popup = Popup {
            pointer: pointer.clone(),
            generation: 0,
            at: Point::new(2, 0),
            markdown: false,
            diagnostics: String::new(),
            text: (0..20).map(|n| format!("line {n}\n")).collect(),
        };
        let size = Point::new(30, 8);
        let first = popup.rows(size);
        // До первого входа действует запас времени даже для длинной подсказки.
        {
            let mut p = pointer.borrow_mut();
            assert!(p.motion(Point::new(31, 9)));
            assert!(p.active.as_ref().unwrap().leaving.is_some());
            p.active.as_mut().unwrap().leaving = None;
        }
        let inside = Point::new(first[1].1 + 1, first[1].0);
        let mut raw = HeadlessBackend::new(30, 8);
        for (pos, wheel) in [(inside, 1), (inside, 127), (inside, -127), (Point::new(31, 9), 0)] {
            raw.input.push_back(RawEvent::Mouse { pos, buttons: 0, mods: Mods::NONE, wheel });
        }
        let mut backend = HoverBackend::new(Box::new(raw), pointer.clone());
        assert!(backend.read(Duration::ZERO).is_none());
        assert!(popup.rows(size)[1].2.contains("line 3"));
        assert!(backend.read(Duration::ZERO).is_none());
        let bottom = popup.rows(size);
        assert!(bottom[6].2.contains("line 19"));
        assert!(bottom[6].2.ends_with('█'));
        assert!(first[1].2.ends_with('█'));
        assert!(backend.read(Duration::ZERO).is_none());
        assert_eq!(popup.rows(size), first);
        assert_eq!(ed.fe.editor.delta, Point::new(0, 0));
        backend.read(Duration::ZERO);
        assert!(!popup.rows(size).is_empty());
        pointer.borrow_mut().active.as_mut().unwrap().leaving = Some(Instant::now() - Duration::from_millis(301));
        pointer.borrow_mut().expire();
        assert!(popup.rows(size).is_empty());
    }

    /// Короткая подсказка удерживается над всем именем и в рамке. Переход
    /// через промежуток допускает 300 мс, возврат отменяет скрытие.
    #[test]
    fn source_and_popup_allow_delayed_transition() {
        let pointer = SharedPointer::default();
        let mut ed = CodeEditor::new(Rect::new(0, 0, 30, 8), None, None, None, std::path::Path::new(""));
        ed.fe.editor.insert_text("helper", false);
        pointer.borrow_mut().pos = Some(Point::new(2, 0));
        activate(&pointer, &ed, Pos::new(0, 2), Point::new(2, 0));
        let popup =
            Popup { pointer: pointer.clone(), generation: 0, at: Point::new(2, 0), markdown: false, diagnostics: String::new(), text: "docs".into() };
        let rows = popup.rows(Point::new(30, 8));
        let inside = Point::new(rows[1].1 + 1, rows[1].0);
        let mut p = pointer.borrow_mut();
        assert!(p.motion(Point::new(5, 0)));
        assert!(p.active.as_ref().unwrap().leaving.is_none());
        assert!(p.motion(Point::new(20, 0)));
        assert!(p.active.as_ref().unwrap().leaving.is_some());
        assert!(p.motion(inside));
        assert!(p.active.as_ref().unwrap().leaving.is_none());
        assert!(p.motion(Point::new(5, 0)));
        assert!(p.active.as_ref().unwrap().leaving.is_none());
        assert!(p.motion(Point::new(20, 0)));
        p.active.as_mut().unwrap().leaving = Some(Instant::now() - Duration::from_millis(301));
        p.expire();
        assert!(p.active.is_none());
        assert_ne!(p.generation, 0);
    }
    fn diag(start: (usize, usize), end: (usize, usize), severity: Severity, message: &str) -> LspDiagnostic {
        LspDiagnostic {
            range: crate::lsp::LRange { start: crate::lsp::LPos { line: start.0, col: start.1 }, end: crate::lsp::LPos { line: end.0, col: end.1 } },
            severity,
            message: message.into(),
            code: "E7".into(),
            source: "test".into(),
        }
    }
    /// UTF-16, табуляция, широкие символы, пустые диапазоны и исключённый конец
    /// используют одни координаты для подсветки, попадания мыши и области удержания.
    #[test]
    fn diagnostics_hit_whitespace_unicode_eol_and_multiline() {
        let mut ed = CodeEditor::new(Rect::new(0, 0, 40, 8), None, None, None, std::path::Path::new(""));
        ed.fe.editor.insert_text("\t😀 x\n\nend", false);
        let tab = tvision::widgets::editor::TAB_SIZE as i32;
        for d in [
            diag((0, 0), (0, 1), Severity::Warning, "tab"),
            diag((0, 1), (0, 3), Severity::Error, "emoji"),
            diag((0, 3), (0, 4), Severity::Warning, "space"),
            diag((0, 5), (0, 5), Severity::Error, "eol"),
            diag((1, 0), (1, 0), Severity::Error, "empty"),
        ] {
            ed.hover_diags.push(Diagnostic::new(&ed, &d, Encoding::Utf16));
        }
        for (point, message) in [
            (Point::new(2, 0), "tab"),
            (Point::new(tab + 1, 0), "emoji"),
            (Point::new(tab + 2, 0), "space"),
            (Point::new(tab + 4, 0), "eol"),
            (Point::new(0, 1), "empty"),
        ] {
            let at = position_at(&ed, point).unwrap();
            assert!(diagnostic_text(&ed, at).ends_with(message));
            let pointer = SharedPointer::default();
            pointer.borrow_mut().pos = Some(point);
            activate(&pointer, &ed, at, point);
            assert!(pointer.borrow().active.as_ref().unwrap().source.iter().any(|r| r.contains(point)));
        }
        assert!(position_at(&ed, Point::new(tab + 5, 0)).is_none());
        let multi = Diagnostic::new(&ed, &diag((0, 3), (2, 0), Severity::Error, "multi"), Encoding::Utf16);
        assert_eq!(multi.spans.len(), 2);
        assert!(!multi.contains(Pos::new(2, 0)), "конечная строка не включена");
        ed.hover_diags = vec![multi];
        let pointer = SharedPointer::default();
        pointer.borrow_mut().pos = Some(Point::new(tab + 2, 0));
        activate(&pointer, &ed, Pos::new(0, 2), Point::new(tab + 2, 0));
        assert!(pointer.borrow_mut().motion(Point::new(0, 1)));
        assert!(pointer.borrow().active.as_ref().unwrap().leaving.is_none());
    }
    /// Диагностики сортируются, сохраняют Markdown буквально и не придают
    /// ему стиль, в то время как нижняя часть той же подсказки разбирает Markdown.
    #[test]
    fn overlapping_diagnostics_are_plaintext_before_markdown() {
        let mut ed = CodeEditor::new(Rect::new(0, 0, 80, 20), None, None, None, std::path::Path::new(""));
        ed.fe.editor.insert_text("name", false);
        for d in [diag((0, 0), (0, 4), Severity::Warning, "**warning**"), diag((0, 1), (0, 3), Severity::Error, "`error`")] {
            ed.hover_diags.push(Diagnostic::new(&ed, &d, Encoding::Utf32));
        }
        let diagnostics = diagnostic_text(&ed, Pos::new(0, 2));
        assert!(diagnostics.starts_with("Ошибка (test · E7)\n`error`"));
        assert!(diagnostics.ends_with("Предупреждение (test · E7)\n**warning**"));
        let popup =
            Popup { pointer: SharedPointer::default(), generation: 0, at: Point::new(0, 0), text: "**hover**".into(), markdown: true, diagnostics };
        let rows = popup.rows(Point::new(80, 20));
        let diagnostic = rows.iter().find(|r| r.2.contains("**warning**")).unwrap();
        assert!(diagnostic.3.iter().all(|s| *s == Style::default()));
        let hover = rows.iter().find(|r| r.2.contains("hover")).unwrap();
        assert!(!hover.2.contains("**"));
        assert!(hover.3.iter().any(|s| *s != Style::default()));
    }
}
