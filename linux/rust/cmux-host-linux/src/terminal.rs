use gtk4 as gtk;
use gtk::glib;
use gtk::gio::Cancellable;
use vte4::{Pty, PtyFlags, Terminal};
use vte4::prelude::*;

use std::process::Command;
use std::sync::OnceLock;

/// Find a monospace font that includes powerline glyphs (U+E0B0).
/// VTE/Pango doesn't fall back to other fonts for Private Use Area characters,
/// so we need to use a font that already contains them. Falls back to "Monospace"
/// if no powerline-capable font is found.
fn powerline_font_family() -> &'static str {
    static FONT: OnceLock<String> = OnceLock::new();
    FONT.get_or_init(|| {
        // Query fontconfig for monospace fonts (spacing=100) that contain U+E0B0
        if let Ok(output) = Command::new("fc-list")
            .args([":charset=e0b0:spacing=100", "family"])
            .output()
        {
            let result = String::from_utf8_lossy(&output.stdout);
            // Prefer well-known fonts in priority order
            let preferred = [
                "Noto Mono for Powerline",
                "DejaVu Sans Mono for Powerline",
                "Fira Mono for Powerline",
                "Source Code Pro for Powerline",
                "Hack",
                "Liberation Mono for Powerline",
                "Meslo LG S for Powerline",
                "Droid Sans Mono for Powerline",
                "Roboto Mono for Powerline",
            ];
            let available: Vec<&str> = result.lines()
                .filter_map(|l| {
                    let family = l.trim().split(',').next()?.trim();
                    if family.is_empty() { None } else { Some(family) }
                })
                .collect();

            for pref in &preferred {
                if available.iter().any(|a| a.eq_ignore_ascii_case(pref)) {
                    eprintln!("cmux: using powerline font: {pref}");
                    return pref.to_string();
                }
            }
            // Use the first available monospace powerline font
            if let Some(first) = available.first() {
                eprintln!("cmux: using powerline font: {first}");
                return first.to_string();
            }
        }
        eprintln!("cmux: no powerline font found, using Monospace (powerline glyphs may not render)");
        "Monospace".to_string()
    })
}

/// Create a new VTE terminal with a shell spawned inside it.
pub fn create_terminal(working_directory: Option<&str>) -> Terminal {
    let terminal = Terminal::new();
    terminal.set_scroll_on_output(false);
    terminal.set_scroll_on_keystroke(true);
    terminal.set_scrollback_lines(10_000);
    terminal.set_hexpand(true);
    terminal.set_vexpand(true);

    // Font — use a powerline-capable monospace font if available.
    // VTE doesn't fall back to other fonts for Private Use Area glyphs
    // (unlike Ghostty which draws powerline symbols as vector graphics).
    let font_family = powerline_font_family();
    let font_str = format!("{font_family} 11");
    let font_desc = gtk::pango::FontDescription::from_string(&font_str);
    terminal.set_font(Some(&font_desc));
    terminal.set_bold_is_bright(true);

    // Colors — dark terminal background
    let bg = gtk::gdk::RGBA::new(0.09, 0.09, 0.09, 1.0);
    let fg = gtk::gdk::RGBA::new(0.9, 0.9, 0.9, 1.0);
    terminal.set_color_background(&bg);
    terminal.set_color_foreground(&fg);

    // Cursor
    terminal.set_cursor_blink_mode(vte4::CursorBlinkMode::On);
    terminal.set_cursor_shape(vte4::CursorShape::Block);

    // Spawn shell
    spawn_shell(&terminal, working_directory);

    terminal
}

fn spawn_shell(terminal: &Terminal, working_directory: Option<&str>) {
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string());

    let pty = match Pty::new_sync(PtyFlags::DEFAULT, None::<&Cancellable>) {
        Ok(pty) => pty,
        Err(e) => {
            eprintln!("Failed to create PTY: {e}");
            return;
        }
    };

    let argv: Vec<&str> = vec![shell.as_str()];
    let envv: Vec<&str> = vec![];
    let working_dir = working_directory.map(|d| d.to_string());

    pty.spawn_async(
        working_dir.as_deref(),
        &argv,
        &envv,
        glib::SpawnFlags::SEARCH_PATH,
        || {},
        -1,
        None::<&Cancellable>,
        |result| {
            if let Err(e) = result {
                eprintln!("Failed to spawn shell: {e}");
            }
        },
    );

    terminal.set_pty(Some(&pty));
}
