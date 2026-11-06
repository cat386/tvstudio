//! «Гаджеты» (`gadgets.cpp`): часы в строке меню и счётчик памяти в строке
//! состояния. В оригинале они обновлялись в `idle()`; здесь экран
//! перерисовывается на каждом шаге цикла, поэтому они просто читают текущие
//! значения при отрисовке.

use tvision::*;

/// Часы `ЧЧ:ММ:СС` (`TClockView`).
pub struct ClockView {
    pub vs: ViewState,
}

impl ClockView {
    pub fn new(r: Rect) -> Self {
        let mut vs = ViewState::new(r);
        vs.grow_mode = gf::GROW_LO_X | gf::GROW_HI_X;
        ClockView { vs }
    }
}

impl View for ClockView {
    tvision::impl_view_basics!();

    fn draw(&self, dc: &mut DrawCtx) {
        let c = dc.color(2);
        let t = platform::time::now();
        let w = self.vs.size.x.max(0) as usize;
        let mut b = DrawBuffer::new(w);
        b.move_char(0, ' ', c, w);
        b.move_str(0, &format!("{:02}:{:02}:{:02}", t.hour, t.minute, t.second), c);
        dc.write_line(0, 0, self.vs.size.x, 1, &b);
    }
}

/// Память процесса (`THeapView`). В оригинале — свободная память DOS или
/// занятая `malloc`; здесь — память процесса по данным ОС.
pub struct HeapView {
    pub vs: ViewState,
}

impl HeapView {
    pub fn new(r: Rect) -> Self {
        let mut vs = ViewState::new(r);
        vs.grow_mode = gf::GROW_ALL;
        HeapView { vs }
    }
}

/// Сколько памяти занимает процесс (в байтах), если ОС это сообщает.
pub fn memory_usage() -> Option<u64> {
    #[cfg(target_os = "linux")]
    {
        // /proc/self/statm: второе число — резидентные страницы.
        let s = std::fs::read_to_string("/proc/self/statm").ok()?;
        let pages: u64 = s.split_whitespace().nth(1)?.parse().ok()?;
        Some(pages * 4096)
    }
    #[cfg(windows)]
    {
        win::private_bytes()
    }
    #[cfg(not(any(target_os = "linux", windows)))]
    {
        None
    }
}

#[cfg(windows)]
mod win {
    #![allow(non_snake_case)]
    use std::os::raw::c_void;

    #[repr(C)]
    #[derive(Default)]
    struct PROCESS_MEMORY_COUNTERS {
        cb: u32,
        PageFaultCount: u32,
        PeakWorkingSetSize: usize,
        WorkingSetSize: usize,
        QuotaPeakPagedPoolUsage: usize,
        QuotaPagedPoolUsage: usize,
        QuotaPeakNonPagedPoolUsage: usize,
        QuotaNonPagedPoolUsage: usize,
        PagefileUsage: usize,
        PeakPagefileUsage: usize,
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn GetCurrentProcess() -> *mut c_void;
        fn K32GetProcessMemoryInfo(p: *mut c_void, c: *mut PROCESS_MEMORY_COUNTERS, cb: u32) -> i32;
    }

    /// Частная память процесса (как `PrivateUsage` в оригинале).
    pub fn private_bytes() -> Option<u64> {
        let mut c = PROCESS_MEMORY_COUNTERS { cb: std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32, ..Default::default() };
        let ok = unsafe { K32GetProcessMemoryInfo(GetCurrentProcess(), &mut c, c.cb) };
        if ok != 0 {
            Some(c.PagefileUsage as u64)
        } else {
            None
        }
    }
}

impl View for HeapView {
    tvision::impl_view_basics!();

    fn draw(&self, dc: &mut DrawCtx) {
        let c = dc.color(2);
        let w = self.vs.size.x.max(0) as usize;
        let mut b = DrawBuffer::new(w);
        b.move_char(0, ' ', c, w);
        let s = match memory_usage() {
            Some(m) => format!("{:>12}", m),
            None => format!("{:>12}", "n/a"),
        };
        b.move_str(0, &s, c);
        dc.write_line(0, 0, self.vs.size.x, 1, &b);
    }
}
