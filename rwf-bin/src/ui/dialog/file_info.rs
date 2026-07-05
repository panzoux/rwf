//! File information dialog rendering.
//!
//! Split from dialog/mod.rs in M3 (move-only; snapshot-protected).

use ratatui::{
    layout::Rect,
    style::{Color, Style},
    widgets::Paragraph,
    Frame,
};

use crate::ui::smart_truncate;

fn fmt_size(bytes: u64) -> String {
    const KB: u64 = 1_024;
    const MB: u64 = 1_024 * KB;
    const GB: u64 = 1_024 * MB;
    if bytes >= GB {
        format!("{:.2} GB ({} bytes)", bytes as f64 / GB as f64, bytes)
    } else if bytes >= MB {
        format!("{:.2} MB ({} bytes)", bytes as f64 / MB as f64, bytes)
    } else if bytes >= KB {
        format!("{:.1} KB ({} bytes)", bytes as f64 / KB as f64, bytes)
    } else {
        format!("{} bytes", bytes)
    }
}

fn fmt_time(t: Option<std::time::SystemTime>) -> String {
    match t {
        None => "N/A".to_string(),
        Some(st) => {
            let dt: chrono::DateTime<chrono::Local> = st.into();
            dt.format("%Y-%m-%d %H:%M:%S").to_string()
        }
    }
}

#[allow(unused_variables, unused_mut)]
#[allow(clippy::too_many_arguments)]
pub(super) fn render_file_info_dialog(
    frame: &mut Frame,
    area: Rect,
    file_name: &str,
    file_path: &str,
    size: u64,
    created: Option<std::time::SystemTime>,
    modified: std::time::SystemTime,
    accessed: Option<std::time::SystemTime>,
    is_dir: bool,
    is_readonly: bool,
    #[cfg(unix)] permissions: Option<u32>,
    #[cfg(unix)] owner: Option<&str>,
    #[cfg(unix)] group: Option<&str>,
    link_target: Option<&str>,
    link_kind: Option<&rwf_lib::model::LinkKind>,
) {
    let base = Style::default().fg(Color::Black).bg(Color::Gray);
    let label = Style::default().fg(Color::DarkGray).bg(Color::Gray);
    let hint = Style::default().fg(Color::DarkGray).bg(Color::Gray);
    let w = area.width.saturating_sub(4) as usize;

    let type_label = match link_kind {
        Some(rwf_lib::model::LinkKind::Junction) => "Junction",
        Some(rwf_lib::model::LinkKind::Symlink) => "Symlink",
        None if is_dir => "Directory",
        None => "File",
    };
    let type_str = if is_readonly {
        format!("{} (Read-only)", type_label)
    } else {
        type_label.to_string()
    };

    let mut rows: Vec<(&str, String)> = vec![
        ("Name", smart_truncate(file_name, w.saturating_sub(8), "…")),
        ("Path", smart_truncate(file_path, w.saturating_sub(8), "…")),
        ("Size", fmt_size(size)),
        ("Type", type_str),
    ];

    if let Some(target) = link_target {
        rows.push(("Target", smart_truncate(target, w.saturating_sub(8), "…")));
    }

    rows.push(("", String::new()));
    rows.push(("Created", fmt_time(created)));
    rows.push(("Modified", fmt_time(Some(modified))));
    rows.push(("Accessed", fmt_time(accessed)));

    let col_w = 9u16; // label column width ("Modified" = 8 chars + space)
    for (row_i, (lbl, val)) in rows.iter().enumerate() {
        let y = area.y + row_i as u16;
        if y + 1 >= area.y + area.height {
            break;
        }
        if lbl.is_empty() {
            continue;
        }
        frame.render_widget(
            Paragraph::new(format!("{:<col_w$}", lbl, col_w = col_w as usize)).style(label),
            Rect::new(area.x + 2, y, col_w, 1),
        );
        frame.render_widget(
            Paragraph::new(val.as_str()).style(base),
            Rect::new(
                area.x + 2 + col_w,
                y,
                w.saturating_sub(col_w as usize) as u16,
                1,
            ),
        );
    }

    // Hint line
    let hint_y = area.y + area.height.saturating_sub(1);
    frame.render_widget(
        Paragraph::new("Enter/Esc: close").style(hint),
        Rect::new(area.x + 2, hint_y, w as u16, 1),
    );
}
