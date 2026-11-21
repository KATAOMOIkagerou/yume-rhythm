//! Windows 原生文件打开 / 保存对话框（基于 COM `IFileOpenDialog` / `IFileSaveDialog`）。

use std::path::PathBuf;
use windows::Win32::Foundation::HWND;
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE, CoCreateInstance,
    CoInitializeEx, CoTaskMemFree, CoUninitialize,
};
use windows::Win32::UI::Shell::Common::COMDLG_FILTERSPEC;
use windows::Win32::UI::Shell::{
    FOS_FILEMUSTEXIST, FOS_FORCEFILESYSTEM, FOS_OVERWRITEPROMPT, FOS_PATHMUSTEXIST, FileOpenDialog,
    FileSaveDialog, IFileOpenDialog, IFileSaveDialog, SIGDN_FILESYSPATH,
};
use windows::core::{HSTRING, PCWSTR};

struct ComGuard {
    should_uninit: bool,
}

impl ComGuard {
    fn enter() -> Self {
        // SAFETY: 当前线程初始化 STA COM；若已被 Slint/Win32 初始化（RPC_E_CHANGED_MODE）则不反初始化。
        let hr = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) };
        Self {
            should_uninit: hr.is_ok(),
        }
    }
}

impl Drop for ComGuard {
    fn drop(&mut self) {
        if self.should_uninit {
            // SAFETY: 与成功配对的 `CoInitializeEx` 对应。
            unsafe { CoUninitialize() };
        }
    }
}

/// 弹出原生「打开文件」对话框。用户取消或出错时返回 `None`。
#[must_use]
pub fn pick_open_file(
    owner: Option<isize>,
    title: &str,
    filter_name: &str,
    filter_spec: &str,
) -> Option<PathBuf> {
    let _com = ComGuard::enter();
    let title_w = HSTRING::from(title);
    let name_w = HSTRING::from(filter_name);
    let spec_w = HSTRING::from(filter_spec);
    let filter = [COMDLG_FILTERSPEC {
        pszName: PCWSTR(name_w.as_ptr()),
        pszSpec: PCWSTR(spec_w.as_ptr()),
    }];
    let hwnd = owner_hwnd(owner);

    // SAFETY: 所有字符串切片与 `filter` 在 `Show` 期间存活；COM 返回路径经 `CoTaskMemFree` 释放。
    unsafe {
        let dialog: IFileOpenDialog =
            CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER).ok()?;
        let _ = dialog.SetTitle(&title_w);
        let _ = dialog.SetFileTypes(&filter);
        let opts = dialog.GetOptions().unwrap_or_default();
        let _ = dialog.SetOptions(opts | FOS_FORCEFILESYSTEM | FOS_FILEMUSTEXIST);
        dialog.Show(hwnd).ok()?;
        let item = dialog.GetResult().ok()?;
        let raw = item.GetDisplayName(SIGDN_FILESYSPATH).ok()?;
        let path = raw.to_string().ok().map(PathBuf::from);
        CoTaskMemFree(Some(raw.0.cast()));
        path
    }
}

/// 弹出原生「保存文件」对话框。用户取消或出错时返回 `None`。
#[must_use]
pub fn pick_save_file(
    owner: Option<isize>,
    title: &str,
    default_name: &str,
    default_ext: &str,
    filter_name: &str,
    filter_spec: &str,
) -> Option<PathBuf> {
    let _com = ComGuard::enter();
    let title_w = HSTRING::from(title);
    let default_name_w = HSTRING::from(default_name);
    let default_ext_w = HSTRING::from(default_ext);
    let name_w = HSTRING::from(filter_name);
    let spec_w = HSTRING::from(filter_spec);
    let filter = [COMDLG_FILTERSPEC {
        pszName: PCWSTR(name_w.as_ptr()),
        pszSpec: PCWSTR(spec_w.as_ptr()),
    }];
    let hwnd = owner_hwnd(owner);

    // SAFETY: 所有字符串切片与 `filter` 在 `Show` 期间存活；COM 返回路径经 `CoTaskMemFree` 释放。
    unsafe {
        let dialog: IFileSaveDialog =
            CoCreateInstance(&FileSaveDialog, None, CLSCTX_INPROC_SERVER).ok()?;
        let _ = dialog.SetTitle(&title_w);
        let _ = dialog.SetFileName(&default_name_w);
        let _ = dialog.SetDefaultExtension(&default_ext_w);
        let _ = dialog.SetFileTypes(&filter);
        let opts = dialog.GetOptions().unwrap_or_default();
        let _ =
            dialog.SetOptions(opts | FOS_FORCEFILESYSTEM | FOS_PATHMUSTEXIST | FOS_OVERWRITEPROMPT);
        dialog.Show(hwnd).ok()?;
        let item = dialog.GetResult().ok()?;
        let raw = item.GetDisplayName(SIGDN_FILESYSPATH).ok()?;
        let path = raw.to_string().ok().map(PathBuf::from);
        CoTaskMemFree(Some(raw.0.cast()));
        path
    }
}

fn owner_hwnd(owner: Option<isize>) -> Option<HWND> {
    owner
        .filter(|&h| h != 0)
        .map(|h| HWND(h as *mut core::ffi::c_void))
}
