//! Индикатор текущего источника ввода. Данные читаются только у локальной ОС.

use std::time::{Duration, Instant};

#[cfg(target_os = "linux")]
mod x11;

pub(crate) struct KeyboardLayout {
    enabled: bool,
    checked_at: Option<Instant>,
    label: Option<String>,
    #[cfg(target_os = "linux")]
    x11: Option<x11::Keyboard>,
}

impl KeyboardLayout {
    pub(crate) fn new() -> Self {
        let enabled = ["SSH_CONNECTION", "SSH_CLIENT", "SSH_TTY"].iter().all(|name| std::env::var_os(name).is_none());
        Self {
            enabled,
            checked_at: None,
            label: None,
            #[cfg(target_os = "linux")]
            x11: if enabled { x11::Keyboard::open() } else { None },
        }
    }

    pub(crate) fn label(&mut self) -> Option<&str> {
        if !self.enabled {
            return None;
        }
        let now = Instant::now();
        if self.checked_at.is_none_or(|last| now.duration_since(last) >= Duration::from_millis(300)) {
            self.checked_at = Some(now);
            self.label = self.read();
        }
        self.label.as_deref()
    }

    #[cfg(test)]
    pub(crate) fn set_test_label(&mut self, label: &str) {
        self.enabled = true;
        self.checked_at = Some(Instant::now());
        self.label = Some(label.to_owned());
    }

    fn read(&mut self) -> Option<String> {
        #[cfg(any(windows, target_os = "macos"))]
        return native::language().as_deref().and_then(language_label);
        #[cfg(target_os = "linux")]
        return self.x11.as_mut()?.layout().as_deref().and_then(x11_label);
        #[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
        None
    }
}

#[cfg(any(windows, target_os = "macos", target_os = "linux", test))]
fn language_label(source: &str) -> Option<String> {
    let language = source.split(['-', '_']).next()?.to_ascii_lowercase();
    match language.as_str() {
        "en" | "eng" => Some("ENG".into()),
        "ru" | "rus" => Some("РУС".into()),
        "uk" | "ukr" => Some("УКР".into()),
        "und" => None,
        _ if (2..=3).contains(&language.len()) && language.bytes().all(|b| b.is_ascii_alphabetic()) => Some(language.to_ascii_uppercase()),
        _ => None,
    }
}

#[cfg(target_os = "linux")]
fn x11_label(name: &str) -> Option<String> {
    let language = name.split(|c: char| !c.is_ascii_alphabetic()).next()?.to_ascii_lowercase();
    let code = match language.as_str() {
        "english" => "en",
        "russian" => "ru",
        "ukrainian" => "uk",
        "group" | "unknown" => return None,
        other => other,
    };
    language_label(code).or_else(|| (code.len() >= 3).then(|| code[..3].to_ascii_uppercase()))
}

#[cfg(windows)]
mod native {
    use std::ffi::c_void;
    use std::ptr;

    #[link(name = "user32")]
    extern "system" {
        fn GetForegroundWindow() -> *mut c_void;
        fn GetWindowThreadProcessId(window: *mut c_void, process_id: *mut u32) -> u32;
        fn GetKeyboardLayout(thread_id: u32) -> *mut c_void;
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn GetLocaleInfoW(locale: u32, kind: u32, text: *mut u16, len: i32) -> i32;
    }

    pub(super) fn language() -> Option<String> {
        const LOCALE_SISO639LANGNAME: u32 = 0x59;
        // У консольной программы свой поток, а ввод получает активное окно терминала.
        // HWND проверяем на каждом чтении: фокус может смениться между опросами.
        unsafe {
            let window = GetForegroundWindow();
            if window.is_null() {
                return None;
            }
            let thread = GetWindowThreadProcessId(window, ptr::null_mut());
            if thread == 0 {
                return None;
            }
            let hkl = GetKeyboardLayout(thread);
            if hkl.is_null() || GetForegroundWindow() != window {
                return None;
            }
            let lang_id = hkl as usize as u32 & 0xffff;
            let mut text = [0u16; 16];
            let len = GetLocaleInfoW(lang_id, LOCALE_SISO639LANGNAME, text.as_mut_ptr(), text.len() as i32);
            (len > 1).then(|| String::from_utf16_lossy(&text[..(len - 1) as usize]))
        }
    }
}

#[cfg(target_os = "macos")]
mod native {
    use std::ffi::{c_char, c_void, CStr};

    type Cf = *const c_void;

    #[link(name = "Carbon", kind = "framework")]
    extern "C" {
        static kTISPropertyInputSourceLanguages: Cf;
        fn TISCopyCurrentKeyboardInputSource() -> Cf;
        fn TISGetInputSourceProperty(source: Cf, key: Cf) -> Cf;
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn CFArrayGetCount(array: Cf) -> isize;
        fn CFArrayGetValueAtIndex(array: Cf, index: isize) -> Cf;
        fn CFStringGetCString(string: Cf, buffer: *mut c_char, capacity: isize, encoding: u32) -> u8;
        fn CFRelease(object: Cf);
    }

    pub(super) fn language() -> Option<String> {
        const UTF8: u32 = 0x08000100;
        // Источник принадлежит вызывающему коду. Массив языков и строки
        // заимствованы у источника и читаются до CFRelease.
        unsafe {
            let source = TISCopyCurrentKeyboardInputSource();
            if source.is_null() {
                return None;
            }
            let languages = TISGetInputSourceProperty(source, kTISPropertyInputSourceLanguages);
            let mut buffer = [0 as c_char; 64];
            let language = if !languages.is_null() && CFArrayGetCount(languages) > 0 {
                let string = CFArrayGetValueAtIndex(languages, 0);
                if !string.is_null() && CFStringGetCString(string, buffer.as_mut_ptr(), buffer.len() as isize, UTF8) != 0 {
                    CStr::from_ptr(buffer.as_ptr()).to_str().ok().map(str::to_owned)
                } else {
                    None
                }
            } else {
                None
            };
            CFRelease(source);
            language
        }
    }
}

#[cfg(test)]
mod tests {
    use super::language_label;

    #[test]
    fn language_labels_fit_indicator() {
        assert_eq!(language_label("en-US").as_deref(), Some("ENG"));
        assert_eq!(language_label("ru_RU").as_deref(), Some("РУС"));
        assert_eq!(language_label("uk").as_deref(), Some("УКР"));
        assert_eq!(language_label("ja").as_deref(), Some("JA"));
        assert_eq!(language_label("und"), None);
    }
}
