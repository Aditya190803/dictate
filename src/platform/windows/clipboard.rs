//! Win32 clipboard access (`CF_UNICODETEXT`).

use anyhow::{anyhow, Result};
use std::time::Duration;
use windows_sys::Win32::Foundation::GlobalFree;
use windows_sys::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, GetClipboardData, IsClipboardFormatAvailable, OpenClipboard,
    SetClipboardData,
};
use windows_sys::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};

const CF_UNICODETEXT: u32 = 13;

/// Holds the clipboard open for the lifetime of the value.
///
/// Only one process may own the clipboard at a time and Explorer, browsers, and
/// clipboard managers all grab it briefly, so opening is retried.
struct Clipboard;

impl Clipboard {
    fn open() -> Result<Self> {
        for attempt in 0..10u64 {
            if unsafe { OpenClipboard(std::ptr::null_mut()) } != 0 {
                return Ok(Self);
            }
            std::thread::sleep(Duration::from_millis(10 * (attempt + 1)));
        }
        Err(anyhow!(
            "could not open the Windows clipboard — another application is holding it"
        ))
    }
}

impl Drop for Clipboard {
    fn drop(&mut self) {
        unsafe { CloseClipboard() };
    }
}

/// Replace the clipboard contents with `text`.
pub fn set_text(text: &str) -> Result<()> {
    let mut utf16: Vec<u16> = text.encode_utf16().collect();
    utf16.push(0);
    let bytes = utf16.len() * std::mem::size_of::<u16>();

    let _clipboard = Clipboard::open()?;
    unsafe {
        if EmptyClipboard() == 0 {
            return Err(anyhow!("EmptyClipboard failed"));
        }

        let handle = GlobalAlloc(GMEM_MOVEABLE, bytes);
        if handle.is_null() {
            return Err(anyhow!("GlobalAlloc failed for {bytes} bytes"));
        }

        let ptr = GlobalLock(handle) as *mut u16;
        if ptr.is_null() {
            GlobalFree(handle);
            return Err(anyhow!("GlobalLock failed"));
        }
        std::ptr::copy_nonoverlapping(utf16.as_ptr(), ptr, utf16.len());
        GlobalUnlock(handle);

        // The clipboard takes ownership of the block only when this succeeds.
        if SetClipboardData(CF_UNICODETEXT, handle).is_null() {
            GlobalFree(handle);
            return Err(anyhow!("SetClipboardData failed"));
        }
    }
    Ok(())
}

/// Current clipboard text, or an empty string when it holds no text.
pub fn get_text() -> Result<String> {
    let _clipboard = Clipboard::open()?;
    unsafe {
        if IsClipboardFormatAvailable(CF_UNICODETEXT) == 0 {
            return Ok(String::new());
        }
        let handle = GetClipboardData(CF_UNICODETEXT);
        if handle.is_null() {
            return Ok(String::new());
        }
        let ptr = GlobalLock(handle) as *const u16;
        if ptr.is_null() {
            return Err(anyhow!("GlobalLock failed"));
        }
        let mut len = 0usize;
        while *ptr.add(len) != 0 {
            len += 1;
        }
        let text = String::from_utf16_lossy(std::slice::from_raw_parts(ptr, len));
        GlobalUnlock(handle);
        Ok(text)
    }
}
