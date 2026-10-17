//! Local, modal file selection; picking a file never executes it.

use windows::{
    Win32::{
        Foundation::ERROR_CANCELLED,
        System::Com::{
            CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
            CoTaskMemFree, CoUninitialize,
        },
        UI::{
            Shell::{
                Common::COMDLG_FILTERSPEC, FOS_DONTADDTORECENT, FOS_FILEMUSTEXIST,
                FOS_FORCEFILESYSTEM, FOS_NOCHANGEDIR, FOS_PATHMUSTEXIST, FOS_STRICTFILETYPES,
                FileOpenDialog, IFileOpenDialog, SIGDN_FILESYSPATH,
            },
            WindowsAndMessaging::FindWindowW,
        },
    },
    core::{HRESULT, PCWSTR, w},
};

#[derive(Clone, Copy)]
pub enum FileKind {
    Executable,
    Batch,
    PowerShell,
}

impl FileKind {
    fn filter(self) -> COMDLG_FILTERSPEC {
        let (name, pattern) = match self {
            Self::Executable => (w!("Windows application (*.exe)"), w!("*.exe")),
            Self::Batch => (w!("Batch script (*.bat; *.cmd)"), w!("*.bat;*.cmd")),
            Self::PowerShell => (w!("PowerShell script (*.ps1)"), w!("*.ps1")),
        };
        COMDLG_FILTERSPEC {
            pszName: name,
            pszSpec: pattern,
        }
    }
}

pub fn pick_file(kind: FileKind) -> windows::core::Result<Option<String>> {
    // Winit's UI thread is STA; balance even an already-initialized COM apartment.
    unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }.ok()?;
    let _apartment = Apartment;
    let dialog: IFileOpenDialog =
        unsafe { CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER)? };
    unsafe {
        dialog.SetTitle(w!("Choose an action file"))?;
        dialog.SetFileTypes(&[kind.filter()])?;
        dialog.SetOptions(
            dialog.GetOptions()?
                | FOS_FILEMUSTEXIST
                | FOS_PATHMUSTEXIST
                | FOS_FORCEFILESYSTEM
                | FOS_STRICTFILETYPES
                | FOS_NOCHANGEDIR
                | FOS_DONTADDTORECENT,
        )?;
        let owner = FindWindowW(PCWSTR::null(), w!("ESP32 Media Daemon")).ok();
        if let Err(error) = dialog.Show(owner) {
            return if error.code() == HRESULT::from_win32(ERROR_CANCELLED.0) {
                Ok(None)
            } else {
                Err(error)
            };
        }
        let name = dialog.GetResult()?.GetDisplayName(SIGDN_FILESYSPATH)?;
        let path = name.to_string();
        CoTaskMemFree(Some(name.0.cast()));
        Ok(Some(path?))
    }
}

struct Apartment;

impl Drop for Apartment {
    fn drop(&mut self) {
        unsafe { CoUninitialize() };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picker_filters_match_the_supported_action_types() {
        for (kind, expected) in [
            (FileKind::Executable, "*.exe"),
            (FileKind::Batch, "*.bat;*.cmd"),
            (FileKind::PowerShell, "*.ps1"),
        ] {
            assert_eq!(
                unsafe { kind.filter().pszSpec.to_string() }.unwrap(),
                expected
            );
        }
    }
}
