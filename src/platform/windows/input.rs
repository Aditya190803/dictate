//! Synthetic keyboard input via `SendInput` — the Windows stand-in for ydotool.

use anyhow::{anyhow, Result};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, KEYEVENTF_UNICODE,
    VIRTUAL_KEY, VK_BACK, VK_CONTROL, VK_LWIN, VK_MENU, VK_RETURN, VK_RWIN, VK_SHIFT, VK_TAB,
};

const VK_V: VIRTUAL_KEY = 0x56;

/// Events per `SendInput` call. Large batches are rejected by some targets, and
/// small ones make long transcripts crawl.
const BATCH: usize = 96;

fn key_event(vk: VIRTUAL_KEY, flags: u32) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

fn unicode_event(unit: u16, flags: u32) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: 0,
                wScan: unit,
                dwFlags: KEYEVENTF_UNICODE | flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

fn send(events: &[INPUT]) -> Result<()> {
    if events.is_empty() {
        return Ok(());
    }
    let sent = unsafe {
        SendInput(
            events.len() as u32,
            events.as_ptr(),
            std::mem::size_of::<INPUT>() as i32,
        )
    };
    if sent as usize != events.len() {
        return Err(anyhow!(
            "SendInput delivered {sent} of {} events (input may be blocked by an elevated window)",
            events.len()
        ));
    }
    Ok(())
}

fn tap(vk: VIRTUAL_KEY) -> Result<()> {
    send(&[key_event(vk, 0), key_event(vk, KEYEVENTF_KEYUP)])
}

/// Type `text` into the focused window.
///
/// Printable characters go through `KEYEVENTF_UNICODE`, which bypasses the
/// keyboard layout entirely. Enter and Tab do not work that way — most apps only
/// act on them as real virtual keys — so they are tapped separately.
pub fn type_text(text: &str) -> Result<()> {
    for (index, line) in text.split('\n').enumerate() {
        if index > 0 {
            tap(VK_RETURN)?;
        }
        type_line(line.strip_suffix('\r').unwrap_or(line))?;
    }
    Ok(())
}

fn type_line(line: &str) -> Result<()> {
    let mut batch: Vec<INPUT> = Vec::with_capacity(BATCH);
    for ch in line.chars() {
        if ch == '\t' {
            send(&batch)?;
            batch.clear();
            tap(VK_TAB)?;
            continue;
        }
        let mut buf = [0u16; 2];
        for unit in ch.encode_utf16(&mut buf) {
            batch.push(unicode_event(*unit, 0));
            batch.push(unicode_event(*unit, KEYEVENTF_KEYUP));
        }
        if batch.len() >= BATCH {
            send(&batch)?;
            batch.clear();
        }
    }
    send(&batch)
}

/// Send `count` backspaces to the focused window.
pub fn backspace(count: usize) -> Result<()> {
    let mut batch: Vec<INPUT> = Vec::with_capacity(BATCH);
    for _ in 0..count {
        batch.push(key_event(VK_BACK, 0));
        batch.push(key_event(VK_BACK, KEYEVENTF_KEYUP));
        if batch.len() >= BATCH {
            send(&batch)?;
            batch.clear();
        }
    }
    send(&batch)
}

/// Release modifiers the user may still be holding from the shortcut that
/// triggered us. Without this, Ctrl+V arrives as Ctrl+Shift+V or Win+Ctrl+V.
pub fn release_modifiers() -> Result<()> {
    let events: Vec<INPUT> = [VK_SHIFT, VK_MENU, VK_LWIN, VK_RWIN]
        .iter()
        .map(|vk| key_event(*vk, KEYEVENTF_KEYUP))
        .collect();
    send(&events)
}

/// Send Ctrl+V to the focused window.
pub fn paste() -> Result<()> {
    release_modifiers()?;
    send(&[
        key_event(VK_CONTROL, 0),
        key_event(VK_V, 0),
        key_event(VK_V, KEYEVENTF_KEYUP),
        key_event(VK_CONTROL, KEYEVENTF_KEYUP),
    ])
}
