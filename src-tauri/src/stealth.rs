//! 悬浮窗的「隐身」增强。
//!
//! 目标：尽可能少地把自身暴露给监考、远程桌面一类的监控软件，同时
//! **保证窗口本身对用户可见**（不做纯后台方案）。
//!
//! 这里做三件事：
//! 1. `SetWindowDisplayAffinity(WDA_EXCLUDEFROMCAPTURE)`
//!    —— 录屏、截图、远程桌面串流都拍不到这个窗口，但用户自己看得见。
//! 2. 去掉焦点能力（tauri 的 `set_focusable(false)`，底层是 `WS_EX_NOACTIVATE`）
//!    —— 显示窗口时不夺取前台焦点，点击也不激活。「谁在前台」是监控软件
//!    最容易也最便宜的检测点，这一项收益最大。
//! 3. `WS_EX_TOOLWINDOW` —— 退出 Alt+Tab 切换列表。
//!
//! 只做能做的：进程名、exe 文件、窗口标题仍然可以被枚举，全局快捷键
//! 仍然会被键盘钩子看到，这些在用户态无法隐藏。

#[cfg(target_os = "windows")]
mod imp {
    use windows_sys::Win32::Foundation::{BOOL, HWND};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetWindowLongPtrW, IsWindowVisible, SetWindowDisplayAffinity, SetWindowLongPtrW, SetWindowPos,
        ShowWindow, GWL_EXSTYLE, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOOWNERZORDER,
        SWP_NOSIZE, SWP_NOZORDER, SW_HIDE, SW_SHOWNA, WDA_EXCLUDEFROMCAPTURE, WS_EX_NOACTIVATE,
        WS_EX_TOOLWINDOW,
    };

    /// `WS_EX_TOOLWINDOW`：把窗口从 Alt+Tab 的切换列表里去掉。
    ///
    /// 注意这里**不碰** `WS_EX_NOACTIVATE` —— 那一项交给 tauri 的
    /// `set_focusable(false)` 管理，两边同时改会互相覆盖。
    pub fn remove_from_alt_tab(hwnd: isize) {
        if hwnd == 0 {
            return;
        }
        unsafe {
            let hwnd = hwnd as HWND;
            let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
            let want = ex | (WS_EX_TOOLWINDOW as isize);
            if want != ex {
                SetWindowLongPtrW(hwnd, GWL_EXSTYLE, want);
                // 样式改动要经过一次 SetWindowPos 才会被 DWM 接受；
                // 用 SWP_NOACTIVATE 以免改样式反而把焦点抢过来。
                SetWindowPos(
                    hwnd,
                    std::ptr::null_mut(),
                    0,
                    0,
                    0,
                    0,
                    SWP_FRAMECHANGED
                        | SWP_NOACTIVATE
                        | SWP_NOMOVE
                        | SWP_NOSIZE
                        | SWP_NOZORDER
                        | SWP_NOOWNERZORDER,
                );
            }
        }
    }

    /// 给窗口挂上 `WS_EX_NOACTIVATE`：之后它既不会被激活成前台，
    /// 点击它也不会把焦点从用户的程序上抢走。
    ///
    /// 必须在**显示之前**调用。tauri 的 `set_focusable(false)` 走的是窗口线程
    /// 的异步队列，和紧随其后的 `show()` 存在竞争；这里直接同步改样式更可靠。
    pub fn disable_activation(hwnd: isize) {
        if hwnd == 0 {
            return;
        }
        unsafe {
            let hwnd = hwnd as HWND;
            let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
            let want = ex | (WS_EX_NOACTIVATE as isize);
            if want != ex {
                SetWindowLongPtrW(hwnd, GWL_EXSTYLE, want);
                SetWindowPos(
                    hwnd,
                    std::ptr::null_mut(),
                    0,
                    0,
                    0,
                    0,
                    SWP_FRAMECHANGED
                        | SWP_NOACTIVATE
                        | SWP_NOMOVE
                        | SWP_NOSIZE
                        | SWP_NOZORDER
                        | SWP_NOOWNERZORDER,
                );
            }
        }
    }

    /// 读回当前扩展样式，仅用于诊断日志。
    pub fn ex_style(hwnd: isize) -> isize {
        unsafe { GetWindowLongPtrW(hwnd as HWND, GWL_EXSTYLE) }
    }

    /// 是否已带 `WS_EX_NOACTIVATE`。
    pub fn has_noactivate(hwnd: isize) -> bool {
        ex_style(hwnd) & (WS_EX_NOACTIVATE as isize) != 0
    }

    /// 是否已带 `WS_EX_TOOLWINDOW`。
    pub fn has_toolwindow(hwnd: isize) -> bool {
        ex_style(hwnd) & (WS_EX_TOOLWINDOW as isize) != 0
    }

    /// 直接问 Win32：窗口现在到底可不可见。
    ///
    /// 不能用 tauri 的 `is_visible()`：它读的是 tao 内部缓存的 WindowFlags。
    /// 我们用 `SW_SHOWNA` 自己显示窗口时，tao 并不知道状态变了，缓存里
    /// 仍是 false，老板键会一直判定「当前隐藏」而反复执行「隐藏」分支。
    pub fn is_really_visible(hwnd: isize) -> bool {
        if hwnd == 0 {
            return false;
        }
        unsafe { IsWindowVisible(hwnd as HWND) != 0 }
    }

    /// 隐藏窗口。
    ///
    /// 同样不能用 tauri 的 `hide()`：tao 内部认为窗口还是隐藏状态（我们的
    /// `SW_SHOWNA` 它看不见），于是 `apply_diff` 判定「无变化」直接返回，
    /// 窗口根本不会被收起来。
    pub fn hide_window(hwnd: isize) {
        if hwnd == 0 {
            return;
        }
        unsafe {
            ShowWindow(hwnd as HWND, SW_HIDE);
        }
    }

    /// 让窗口从「屏幕捕获」里消失，但用户肉眼仍能看见。
    /// 需要 Windows 10 2004（build 19041）及以上；老系统上调用失败，
    /// 此时返回 false，退化为「不做捕获保护」，不影响其他功能。
    ///
    /// 注意：对**隐藏**状态的窗口调用会失败，必须在 `show()` 之后调用。
    pub fn exclude_from_capture(hwnd: isize) -> bool {
        if hwnd == 0 {
            return false;
        }
        unsafe {
            let ok: BOOL = SetWindowDisplayAffinity(hwnd as HWND, WDA_EXCLUDEFROMCAPTURE);
            ok != 0
        }
    }

    /// 不夺焦点地把窗口显示出来，绕过 tauri 的 `show()`。
    ///
    /// tauri 的 `show()` 走 tao 的 apply_diff：先 `ShowWindow(SW_SHOW)`（这一步
    /// 就把窗口激活成前台了），之后才重建扩展样式。也就是说等 tauri 返回时
    /// 焦点早被抢走，事后再补 `WS_EX_NOACTIVATE` 已经来不及。
    ///
    /// 这里自己完成整套流程，顺序是：
    ///   1. 先挂 `WS_EX_NOACTIVATE`（让窗口天生不可激活）
    ///   2. `ShowWindow(SW_SHOWNA)` —— 只画出来，不激活
    ///   3. 补 `WS_EX_TOOLWINDOW` 与捕获保护
    ///
    /// 返回 false 表示句柄无效，调用方可以退回 tauri 的 show。
    pub fn reveal_stealth(hwnd: isize) -> bool {
        if hwnd == 0 {
            return false;
        }
        disable_activation(hwnd);
        unsafe {
            ShowWindow(hwnd as HWND, SW_SHOWNA);
        }
        remove_from_alt_tab(hwnd);
        exclude_from_capture(hwnd);
        true
    }
}

#[cfg(not(target_os = "windows"))]
mod imp {
    pub fn remove_from_alt_tab(_hwnd: isize) {}
    pub fn disable_activation(_hwnd: isize) {}
    pub fn exclude_from_capture(_hwnd: isize) -> bool {
        false
    }
    pub fn ex_style(_hwnd: isize) -> isize {
        0
    }
    pub fn has_noactivate(_hwnd: isize) -> bool {
        false
    }
    pub fn has_toolwindow(_hwnd: isize) -> bool {
        false
    }
    pub fn is_really_visible(_hwnd: isize) -> bool {
        false
    }
    pub fn hide_window(_hwnd: isize) {}
    pub fn reveal_stealth(_hwnd: isize) -> bool {
        false
    }
}

pub use imp::*;
