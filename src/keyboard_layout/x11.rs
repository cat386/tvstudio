//! XKB через libxkbcommon-x11. Библиотека нужна только в сеансе X11.

use std::ffi::{c_char, c_int, c_void, CStr};
use std::ptr::{self, NonNull};
use std::time::{Duration, Instant};

type Handle = *mut c_void;
const EFFECTIVE_LAYOUT: u32 = 1 << 7;

#[link(name = "dl")]
extern "C" {
    fn dlopen(filename: *const c_char, flags: c_int) -> Handle;
    fn dlsym(library: Handle, name: *const c_char) -> Handle;
    fn dlclose(library: Handle) -> c_int;
}

struct Library(NonNull<c_void>);

impl Drop for Library {
    fn drop(&mut self) {
        unsafe { dlclose(self.0.as_ptr()) };
    }
}

// Один список связывает имена символов libxkbcommon и XCB с их ABI.
macro_rules! x11_api {
    ($($field:ident: $signature:ty = $symbol:literal),+ $(,)?) => {
        struct Api {
            $($field: $signature,)+
            _library: Library,
        }

        impl Api {
            fn open() -> Option<Self> {
                const RTLD_NOW: c_int = 2;
                let library = Library(NonNull::new(unsafe { dlopen(c"libxkbcommon-x11.so.0".as_ptr(), RTLD_NOW) })?);
                // dlsym ищет символы и в зависимостях библиотеки, включая libxcb.
                Some(Self {
                    $($field: {
                        let pointer = NonNull::new(unsafe { dlsym(library.0.as_ptr(), $symbol.as_ptr()) })?;
                        unsafe { std::mem::transmute::<Handle, $signature>(pointer.as_ptr()) }
                    },)+
                    _library: library,
                })
            }
        }
    };
}

x11_api! {
    connect: unsafe extern "C" fn(*const c_char, *mut c_int) -> Handle = c"xcb_connect",
    connection_error: unsafe extern "C" fn(Handle) -> c_int = c"xcb_connection_has_error",
    disconnect: unsafe extern "C" fn(Handle) = c"xcb_disconnect",
    setup: unsafe extern "C" fn(Handle, u16, u16, c_int, *mut u16, *mut u16, *mut u8, *mut u8) -> c_int = c"xkb_x11_setup_xkb_extension",
    device: unsafe extern "C" fn(Handle) -> i32 = c"xkb_x11_get_core_keyboard_device_id",
    context_new: unsafe extern "C" fn(c_int) -> Handle = c"xkb_context_new",
    context_unref: unsafe extern "C" fn(Handle) = c"xkb_context_unref",
    keymap_new: unsafe extern "C" fn(Handle, Handle, i32, c_int) -> Handle = c"xkb_x11_keymap_new_from_device",
    keymap_unref: unsafe extern "C" fn(Handle) = c"xkb_keymap_unref",
    layout_name: unsafe extern "C" fn(Handle, u32) -> *const c_char = c"xkb_keymap_layout_get_name",
    state_new: unsafe extern "C" fn(Handle, Handle, i32) -> Handle = c"xkb_x11_state_new_from_device",
    state_unref: unsafe extern "C" fn(Handle) = c"xkb_state_unref",
    layout_active: unsafe extern "C" fn(Handle, u32, u32) -> c_int = c"xkb_state_layout_index_is_active",
    layout_count: unsafe extern "C" fn(Handle) -> u32 = c"xkb_keymap_num_layouts",
}

pub(super) struct Keyboard {
    api: Api,
    connection: NonNull<c_void>,
    context: NonNull<c_void>,
    device: i32,
    keymap: Option<NonNull<c_void>>,
    mapped_at: Option<Instant>,
}

impl Keyboard {
    pub(super) fn open() -> Option<Self> {
        // DISPLAY в сеансе Wayland часто указывает на XWayland, где группа
        // может отличаться от раскладки окна, принимающего клавиатуру.
        if std::env::var_os("WAYLAND_DISPLAY").is_some() || std::env::var("XDG_SESSION_TYPE").ok().as_deref() == Some("wayland") {
            return None;
        }
        let display = std::env::var("DISPLAY").ok()?;
        if !(display.starts_with(':') || display.starts_with("unix:")) {
            return None;
        }
        let api = Api::open()?;
        unsafe {
            let connection = NonNull::new((api.connect)(ptr::null(), ptr::null_mut()))?;
            let conn = connection.as_ptr();
            if (api.connection_error)(conn) != 0
                || (api.setup)(conn, 1, 0, 0, ptr::null_mut(), ptr::null_mut(), ptr::null_mut(), ptr::null_mut()) == 0
            {
                (api.disconnect)(conn);
                return None;
            }
            let device = (api.device)(conn);
            if device < 0 {
                (api.disconnect)(conn);
                return None;
            }
            let Some(context) = NonNull::new((api.context_new)(0)) else {
                (api.disconnect)(conn);
                return None;
            };
            Some(Self { api, connection, context, device, keymap: None, mapped_at: None })
        }
    }

    pub(super) fn layout(&mut self) -> Option<String> {
        let now = Instant::now();
        let api = &self.api;
        unsafe {
            // При обычном переключении меняется только состояние. Карту
            // перечитываем реже, чтобы подхватить замену раскладок в настройках.
            if self.mapped_at.is_none_or(|last| now.duration_since(last) >= Duration::from_secs(5)) {
                let new_map = NonNull::new((api.keymap_new)(self.context.as_ptr(), self.connection.as_ptr(), self.device, 0));
                if let Some(old_map) = std::mem::replace(&mut self.keymap, new_map) {
                    (api.keymap_unref)(old_map.as_ptr());
                }
                self.mapped_at = Some(now);
            }
            let map = self.keymap?.as_ptr();
            let state = NonNull::new((api.state_new)(map, self.connection.as_ptr(), self.device))?;
            let group = (0..(api.layout_count)(map)).find(|&index| (api.layout_active)(state.as_ptr(), index, EFFECTIVE_LAYOUT) == 1);
            (api.state_unref)(state.as_ptr());
            let name = (api.layout_name)(map, group?);
            if name.is_null() {
                return None;
            }
            CStr::from_ptr(name).to_str().ok().map(str::to_owned)
        }
    }
}

impl Drop for Keyboard {
    fn drop(&mut self) {
        unsafe {
            if let Some(keymap) = self.keymap {
                (self.api.keymap_unref)(keymap.as_ptr());
            }
            (self.api.context_unref)(self.context.as_ptr());
            (self.api.disconnect)(self.connection.as_ptr());
        }
    }
}
