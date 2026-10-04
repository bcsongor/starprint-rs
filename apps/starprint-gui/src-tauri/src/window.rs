//! Window chrome the web view cannot reach.

/// Paints the title bar to match the dark toolbar under it: shadcn's
/// dark `--background`, `oklch(0.145 0 0)`, which is about `#0a0a0a`.
#[cfg(windows)]
pub fn colour_title_bar(app: &tauri::App) {
    use tauri::Manager;
    use windows_sys::Win32::Graphics::Dwm::{
        DWMWA_BORDER_COLOR, DWMWA_CAPTION_COLOR, DWMWA_TEXT_COLOR, DwmSetWindowAttribute,
    };

    // COLORREF is 0x00BBGGRR.
    const BACKGROUND: u32 = 0x000a0a0a;
    const FOREGROUND: u32 = 0x00fafafa;

    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    let Ok(hwnd) = window.hwnd() else { return };
    for (attribute, colour) in [
        (DWMWA_CAPTION_COLOR, BACKGROUND),
        (DWMWA_BORDER_COLOR, BACKGROUND),
        (DWMWA_TEXT_COLOR, FOREGROUND),
    ] {
        // SAFETY: live window handle; the attribute takes a COLORREF.
        unsafe {
            DwmSetWindowAttribute(
                hwnd.0 as _,
                attribute as u32,
                (&raw const colour).cast(),
                std::mem::size_of::<u32>() as u32,
            );
        }
    }
}

/// Asks GTK for its dark theme, and paints the title bar GTK draws to
/// match the toolbar under it, at 27 px where the theme's is 37. tao
/// takes the theme from the desktop's setting and not from
/// `tauri.conf.json`, so a light desktop would otherwise get a light
/// title bar over the dark app.
///
/// GTK draws the title bar where the compositor leaves it to the
/// window: GNOME on Wayland, and WSLg. Elsewhere the window manager
/// draws it, and only the dark theme reaches it, as a hint.
#[cfg(target_os = "linux")]
pub fn colour_title_bar(_app: &tauri::App) {
    use gtk::prelude::*;

    // `default-decoration` is the title bar GTK gives a window that
    // brought none, so a dialog's own header bar keeps the theme's.
    const CSS: &[u8] = b"
        headerbar.default-decoration {
            min-height: 0;
            padding: 2px 4px;
            background: #0a0a0a;
            border-color: #0a0a0a;
            box-shadow: none;
            color: #fafafa;
        }
        headerbar.default-decoration button.titlebutton {
            min-height: 20px;
            min-width: 20px;
        }
    ";

    if let Some(settings) = gtk::Settings::default() {
        settings.set_gtk_application_prefer_dark_theme(true);
    }
    let provider = gtk::CssProvider::new();
    if provider.load_from_data(CSS).is_ok()
        && let Some(screen) = gtk::gdk::Screen::default()
    {
        gtk::StyleContext::add_provider_for_screen(
            &screen,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}

/// macOS draws its own, and already matches the dark theme.
#[cfg(not(any(windows, target_os = "linux")))]
pub fn colour_title_bar(_app: &tauri::App) {}
