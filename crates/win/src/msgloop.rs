//! 线程消息循环与高精度帧定时。

use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::System::Threading::{
    CREATE_WAITABLE_TIMER_HIGH_RESOLUTION, CreateWaitableTimerExW, INFINITE, SetWaitableTimer,
    TIMER_ALL_ACCESS,
};
use windows::Win32::UI::WindowsAndMessaging as wm;
use windows::core::PCWSTR;

/// 处理本线程待处理的消息，最多 `limit` 条。收到 `WM_QUIT` 时返回 `false`。
///
/// 设上限是为了不被源源不断的消息（拖动时的鼠标移动）困住，调用方可以按时出帧。
pub fn pump_messages(limit: usize) -> bool {
    let mut msg = wm::MSG::default();
    // SAFETY: 标准消息循环，`msg` 是局部变量。
    unsafe {
        for _ in 0..limit {
            if !wm::PeekMessageW(&raw mut msg, None, 0, 0, wm::PM_REMOVE).as_bool() {
                break;
            }
            if msg.message == wm::WM_QUIT {
                return false;
            }
            let _ = wm::TranslateMessage(&raw const msg);
            wm::DispatchMessageW(&raw const msg);
        }
    }
    true
}

/// 让本线程的消息循环退出。
pub fn post_quit() {
    // SAFETY: 无参数要求。
    unsafe { wm::PostQuitMessage(0) };
}

/// 帧定时器：等待到指定时间，或在有新消息时提前醒来。
///
/// 优先使用高精度可等待定时器（Windows 10 1803+），不修改系统全局定时精度。
pub struct FrameTimer {
    handle: HANDLE,
}

impl FrameTimer {
    /// # Errors
    /// 创建定时器失败。
    pub fn new() -> windows::core::Result<Self> {
        let access = TIMER_ALL_ACCESS.0;
        // SAFETY: 创建匿名定时器；失败时退回普通精度。
        let handle = unsafe {
            CreateWaitableTimerExW(
                None,
                PCWSTR::null(),
                CREATE_WAITABLE_TIMER_HIGH_RESOLUTION,
                access,
            )
            .or_else(|_| CreateWaitableTimerExW(None, PCWSTR::null(), 0, access))?
        };
        Ok(Self { handle })
    }

    /// 最多等待 `ms` 毫秒；有窗口消息到达时立即返回。
    pub fn wait(&self, ms: f64) {
        if ms <= 0.0 {
            return;
        }
        #[allow(clippy::cast_possible_truncation, reason = "等待时间很短")]
        let due = -(ms * 10_000.0) as i64;
        // SAFETY: 定时器句柄有效；`due` 是局部变量。
        unsafe {
            if SetWaitableTimer(self.handle, &raw const due, 0, None, None, false).is_err() {
                return;
            }
            let handles = [self.handle];
            let _ = wm::MsgWaitForMultipleObjectsEx(
                Some(&handles),
                INFINITE,
                wm::QS_ALLINPUT,
                wm::MWMO_INPUTAVAILABLE,
            );
        }
    }
}

impl Drop for FrameTimer {
    fn drop(&mut self) {
        // SAFETY: 句柄由本对象创建。
        unsafe {
            let _ = CloseHandle(self.handle);
        }
    }
}
