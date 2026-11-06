//! JSON → таблица — общее для примеров `json_table` и `json_load`.
//!
//! Какие JSON понимаются (см. [`to_table`]):
//! * массив объектов `[{...}, {...}]` — строка на объект, столбец на ключ;
//!   вложенные объекты разворачиваются в столбцы `адрес.город`;
//! * объект с массивом внутри `{"items": [...]}` — берётся первый массив;
//! * объект объектов `{"id1": {...}, "id2": {...}}` — первый столбец «ключ»;
//! * массив массивов `[[1, 2], [3, 4]]` — столбцы A, B, …;
//! * массив значений `[1, 2, 3]` и одиночный объект — тоже показываются.

use super::parser::Json;
use tvision::column_letters;

/// Тип данных столбца — от него зависит выравнивание.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColKind {
    Number,
    Bool,
    Text,
}

/// Данные для таблицы: заголовки, строки, тип столбцов и исходные записи
/// (для окна «Запись»).
#[derive(Clone, Debug, PartialEq)]
pub struct JsonTable {
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub kinds: Vec<ColKind>,
    pub records: Vec<Json>,
    /// Откуда взяты строки: `""` — весь файл, `"items"` — поле объекта.
    pub source: String,
}

/// Развернуть объект в пары «столбец — значение»: вложенные объекты дают
/// столбцы через точку (`адрес.город`), не глубже `depth` уровней.
pub fn flatten(prefix: &str, obj: &[(String, Json)], depth: usize, out: &mut Vec<(String, Json)>) {
    for (k, v) in obj {
        let name = if prefix.is_empty() { k.clone() } else { format!("{}.{}", prefix, k) };
        match v {
            Json::Obj(inner) if depth > 0 && !inner.is_empty() => flatten(&name, inner, depth - 1, out),
            _ => out.push((name, v.clone())),
        }
    }
}

/// Найти массив, который стоит показывать: сам корень или первое поле-массив
/// объекта (`{"data": [...]}`), если оно непустое.
pub fn find_rows(root: &Json) -> (String, Option<&Vec<Json>>) {
    match root {
        Json::Arr(a) => (String::new(), Some(a)),
        Json::Obj(fields) => {
            for (k, v) in fields {
                if let Json::Arr(a) = v {
                    if !a.is_empty() {
                        return (k.clone(), Some(a));
                    }
                }
            }
            (String::new(), None)
        }
        _ => (String::new(), None),
    }
}

/// Превратить JSON в таблицу.
pub fn to_table(root: &Json) -> JsonTable {
    to_table_with(root, &mut |_, _| true).expect("без прерывания")
}

/// То же с сообщениями о ходе: `hook(готово строк, всего строк)` (строки
/// проходятся дважды — сбор столбцов и заполнение ячеек, поэтому «всего» —
/// удвоенное число записей). `None` — прервано.
pub fn to_table_with(root: &Json, hook: &mut dyn FnMut(usize, usize) -> bool) -> Option<JsonTable> {
    let (source, arr) = find_rows(root);
    // Строки таблицы: список (ключ, запись). Ключ есть у «объекта объектов».
    let mut items: Vec<(Option<String>, Json)> = match (arr, root) {
        (Some(a), _) => a.iter().map(|v| (None, v.clone())).collect(),
        // Объект, все значения которого — объекты: строка на значение.
        (None, Json::Obj(f)) if !f.is_empty() && f.iter().all(|(_, v)| matches!(v, Json::Obj(_))) => {
            f.iter().map(|(k, v)| (Some(k.clone()), v.clone())).collect()
        }
        // Одиночный объект или значение — одна строка.
        (None, other) => vec![(None, other.clone())],
    };
    let keyed = items.iter().any(|(k, _)| k.is_some());

    // Столбцы в порядке первого появления ключа.
    let mut headers: Vec<String> = Vec::new();
    let mut rows_kv: Vec<Vec<(String, Json)>> = Vec::new();
    let all_arrays = items.iter().all(|(_, v)| matches!(v, Json::Arr(_)));
    let total = items.len() * 2;
    for (n, (key, v)) in items.iter_mut().enumerate() {
        if n % 1000 == 0 && !hook(n, total) {
            return None;
        }
        let mut kv = Vec::new();
        if let Some(k) = key {
            kv.push(("ключ".to_string(), Json::Str(k.clone())));
        }
        match v {
            Json::Obj(o) => flatten("", o, 3, &mut kv),
            // Массив массивов: столбцы A, B, C…
            Json::Arr(a) if all_arrays => {
                for (i, x) in a.iter().enumerate() {
                    kv.push((column_letters(i), x.clone()));
                }
            }
            other => kv.push(("значение".to_string(), other.clone())),
        }
        for (k, _) in &kv {
            if !headers.contains(k) {
                headers.push(k.clone());
            }
        }
        rows_kv.push(kv);
    }
    if keyed {
        // «ключ» — всегда первый столбец.
        headers.retain(|h| h != "ключ");
        headers.insert(0, "ключ".into());
    }

    // Ячейки и типы столбцов. Отсутствующее поле — пустая ячейка.
    let mut kinds = vec![None::<ColKind>; headers.len()];
    let mut rows: Vec<Vec<String>> = Vec::with_capacity(rows_kv.len());
    let half = rows_kv.len();
    for (n, kv) in rows_kv.iter().enumerate() {
        if n % 1000 == 0 && !hook(half + n, total) {
            return None;
        }
        let row: Vec<String> = headers
            .iter()
            .enumerate()
            .map(|(ci, h)| match kv.iter().find(|(k, _)| k == h) {
                Some((_, v)) => {
                    let k = match v {
                        Json::Num(_) => Some(ColKind::Number),
                        Json::Bool(_) => Some(ColKind::Bool),
                        Json::Null => None,
                        _ => Some(ColKind::Text),
                    };
                    if let Some(k) = k {
                        kinds[ci] = match kinds[ci] {
                            None => Some(k),
                            Some(old) if old == k => Some(k),
                            _ => Some(ColKind::Text),
                        };
                    }
                    v.cell_text()
                }
                None => String::new(),
            })
            .collect();
        rows.push(row);
    }
    hook(total, total);
    let records = items.into_iter().map(|(_, v)| v).collect();
    Some(JsonTable { headers, rows, kinds: kinds.into_iter().map(|k| k.unwrap_or(ColKind::Text)).collect(), records, source })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json::parse_json;

    #[test]
    fn array_of_objects() {
        let j = parse_json(r#"[{"id": 1, "имя": "Аня", "адрес": {"город": "Тула"}}, {"id": 2, "имя": "Борис", "тэги": ["a", "b"], "ok": true}]"#)
            .unwrap();
        let t = to_table(&j);
        assert_eq!(t.headers, vec!["id", "имя", "адрес.город", "тэги", "ok"]);
        assert_eq!(t.rows[0], vec!["1", "Аня", "Тула", "", ""]);
        assert_eq!(t.rows[1], vec!["2", "Борис", "", "a, b", "true"]);
        assert_eq!(t.kinds, vec![ColKind::Number, ColKind::Text, ColKind::Text, ColKind::Text, ColKind::Bool]);
    }

    #[test]
    fn other_shapes() {
        // Объект с массивом внутри.
        let t = to_table(&parse_json(r#"{"total": 2, "items": [{"a": 1}, {"a": 2}]}"#).unwrap());
        assert_eq!((t.source.as_str(), t.rows.len()), ("items", 2));
        // Объект объектов: столбец «ключ».
        let t = to_table(&parse_json(r#"{"x": {"v": 1}, "y": {"v": 2}}"#).unwrap());
        assert_eq!(t.headers, vec!["ключ", "v"]);
        assert_eq!(t.rows[1], vec!["y", "2"]);
        // Массив массивов.
        let t = to_table(&parse_json("[[1, 2], [3, 4, 5]]").unwrap());
        assert_eq!(t.headers, vec!["A", "B", "C"]);
        assert_eq!(t.rows[0], vec!["1", "2", ""]);
        // Массив значений и смешанный тип столбца.
        let t = to_table(&parse_json(r#"[1, "два", null]"#).unwrap());
        assert_eq!(t.headers, vec!["значение"]);
        assert_eq!(t.kinds, vec![ColKind::Text]);
        assert_eq!(t.rows[2], vec!["null"]);
    }
}
