//! System-wide hotkeys (`RegisterHotKey`), the Windows equivalent of a
//! compositor keybinding that runs `dictate toggle live|smart`.

use anyhow::{anyhow, bail, Result};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    RegisterHotKey, UnregisterHotKey, HOT_KEY_MODIFIERS, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT,
    MOD_SHIFT, MOD_WIN,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{GetMessageW, MSG, WM_HOTKEY};

/// A parsed shortcut such as `SUPER,SHIFT,R`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HotkeySpec {
    pub modifiers: HOT_KEY_MODIFIERS,
    pub key: u32,
}

/// Parse a shortcut string.
///
/// Accepts the compositor styles already used in `.env` — `SUPER,R`,
/// `Meta+Shift+R`, and GNOME's `<Super><Shift>r` — so the same config value
/// works on either platform.
pub fn parse(spec: &str) -> Result<HotkeySpec> {
    let normalized = spec.replace('<', "").replace('>', "+");
    let mut modifiers: HOT_KEY_MODIFIERS = MOD_NOREPEAT;
    let mut key = None;

    for part in normalized.split(['+', ',']) {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        match part.to_uppercase().as_str() {
            "SUPER" | "META" | "WIN" | "MOD" | "MOD4" | "CMD" => modifiers |= MOD_WIN,
            "SHIFT" => modifiers |= MOD_SHIFT,
            "CTRL" | "CONTROL" => modifiers |= MOD_CONTROL,
            "ALT" => modifiers |= MOD_ALT,
            name => key = Some(virtual_key(name)?),
        }
    }

    let key = key.ok_or_else(|| anyhow!("shortcut '{spec}' has no key, only modifiers"))?;
    if modifiers == MOD_NOREPEAT {
        bail!("shortcut '{spec}' has no modifier — Windows would capture the bare key globally");
    }
    Ok(HotkeySpec { modifiers, key })
}

fn virtual_key(name: &str) -> Result<u32> {
    if name.len() == 1 {
        let ch = name.as_bytes()[0];
        if ch.is_ascii_alphanumeric() {
            return Ok(ch.to_ascii_uppercase() as u32);
        }
    }
    if let Some(digits) = name.strip_prefix('F') {
        if let Ok(n) = digits.parse::<u32>() {
            if (1..=24).contains(&n) {
                return Ok(0x70 + n - 1);
            }
        }
    }
    Ok(match name {
        "SPACE" => 0x20,
        "ENTER" | "RETURN" => 0x0D,
        "TAB" => 0x09,
        "ESC" | "ESCAPE" => 0x1B,
        "BACKSPACE" => 0x08,
        "INSERT" => 0x2D,
        "DELETE" | "DEL" => 0x2E,
        "HOME" => 0x24,
        "END" => 0x23,
        "PAGEUP" | "PRIOR" => 0x21,
        "PAGEDOWN" | "NEXT" => 0x22,
        "UP" => 0x26,
        "DOWN" => 0x28,
        "LEFT" => 0x25,
        "RIGHT" => 0x27,
        other => bail!("unsupported key '{other}' in shortcut"),
    })
}

/// Register `bindings` and pump messages until the process exits.
///
/// `RegisterHotKey` binds to the calling thread and delivers `WM_HOTKEY` to its
/// message queue, so this must own the thread it runs on. Each binding is an
/// id, the combination, and a label used only in the conflict error.
/// `on_press` receives the id of the binding that fired.
pub fn run_loop<F>(bindings: &[(i32, HotkeySpec, String)], mut on_press: F) -> Result<()>
where
    F: FnMut(i32),
{
    let mut registered: Vec<i32> = Vec::new();
    for (id, spec, label) in bindings {
        let ok = unsafe { RegisterHotKey(std::ptr::null_mut(), *id, spec.modifiers, spec.key) };
        if ok == 0 {
            for id in &registered {
                unsafe { UnregisterHotKey(std::ptr::null_mut(), *id) };
            }
            bail!("'{label}' is already taken by Windows or another application");
        }
        registered.push(*id);
    }

    let mut msg: MSG = unsafe { std::mem::zeroed() };
    loop {
        let result = unsafe { GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) };
        if result <= 0 {
            break;
        }
        if msg.message == WM_HOTKEY {
            on_press(msg.wParam as i32);
        }
    }

    for id in &registered {
        unsafe { UnregisterHotKey(std::ptr::null_mut(), *id) };
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_compositor_style_shortcut() {
        let spec = parse("SUPER,SHIFT,R").unwrap();
        assert_eq!(spec.key, 'R' as u32);
        assert_eq!(spec.modifiers, MOD_NOREPEAT | MOD_WIN | MOD_SHIFT);
    }

    #[test]
    fn parses_gnome_and_plus_styles() {
        assert_eq!(
            parse("<Super><Shift>r").unwrap(),
            parse("SUPER,SHIFT,R").unwrap()
        );
        assert_eq!(
            parse("Meta+Shift+R").unwrap(),
            parse("SUPER,SHIFT,R").unwrap()
        );
    }

    #[test]
    fn parses_function_keys() {
        assert_eq!(parse("CTRL,F9").unwrap().key, 0x78);
    }

    #[test]
    fn rejects_modifierless_and_unknown_keys() {
        assert!(parse("R").is_err());
        assert!(parse("SUPER").is_err());
        assert!(parse("SUPER,BANANA").is_err());
    }
}
