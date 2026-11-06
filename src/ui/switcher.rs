//! The recent-chat switcher (Ctrl+K): jump between the chats opened
//! most recently. Older and never-opened chats stay out on purpose - the
//! command palette (Ctrl+P) and the chat-list search (Ctrl+Shift+F) find
//! those.

use egui::{Key, Modifiers, Sense, vec2};

use super::{theme, widgets};
use crate::app::App;
use crate::model::{Action, ChatId};

/// One rendered row: a recent chat.
pub struct SwitcherEntry {
    pub title: String,
    pub detail: String,
    pub id: ChatId,
    pub current: bool,
}

/// The recent chats matching a query, most recent last. Pure over app
/// state, so the matching is testable without a frame.
///
/// Tabs whose chat is not loaded (or is gone) and locked chats outside
/// the unlocked folder stay out: they cannot be opened from here either.
pub fn entries(app: &App, query: &str) -> Vec<SwitcherEntry> {
    let needle = query.trim().to_lowercase();
    let mut out = Vec::new();
    for id in &app.open_tabs {
        let Some(chat) = app.chat(id) else {
            continue;
        };
        if chat.locked && !app.locked_folder_open() {
            continue;
        }
        let title = app.chat_title(chat);
        if !needle.is_empty() && !title.to_lowercase().contains(&needle) {
            continue;
        }
        let current = app.open_chat.as_ref() == Some(id);
        let network = chat.network.clone().unwrap_or_default();
        let detail = if current {
            let current = crate::i18n::gettext(app.locale, "current");
            if network.is_empty() {
                current.into_owned()
            } else {
                format!("{network} · {current}")
            }
        } else {
            network
        };
        out.push(SwitcherEntry {
            title,
            detail,
            id: id.clone(),
            current,
        });
    }
    out
}

/// Where the highlight starts: the chat after the current one, wrapping,
/// so Enter alone alternates the two most recent chats. Falls back to the
/// first row when the current chat is not in the list.
pub fn default_selected(app: &App, entries: &[SwitcherEntry]) -> usize {
    if entries.is_empty() {
        return 0;
    }
    let Some(current) = &app.open_chat else {
        return 0;
    };
    let Some(pos) = app.open_tabs.iter().position(|tab| tab == current) else {
        return 0;
    };
    if app.open_tabs.len() < 2 {
        return 0;
    }
    let next = (pos + 1) % app.open_tabs.len();
    entries
        .iter()
        .position(|entry| entry.id == app.open_tabs[next])
        .unwrap_or(0)
}

/// Opens the row's chat and closes the switcher.
fn execute(app: &mut App, id: &ChatId) {
    app.actions.push(Action::OpenChat(id.clone()));
    app.actions.push(Action::CloseDialog);
}

/// Renders the switcher dialog.
pub fn show(app: &mut App, ui: &mut egui::Ui) {
    let palette = app.palette;
    let entries = entries(app, &app.jump_query);
    if entries.is_empty() {
        app.jump_selected = 0;
    } else if app.jump_selected >= entries.len() {
        app.jump_selected = entries.len() - 1;
    }

    // Consume navigation before the query field can move its caret.
    let down = ui.input_mut(|input| input.consume_key(Modifiers::NONE, Key::ArrowDown));
    let up = ui.input_mut(|input| input.consume_key(Modifiers::NONE, Key::ArrowUp));
    let enter = ui.input_mut(|input| input.consume_key(Modifiers::NONE, Key::Enter));
    if down && !entries.is_empty() {
        app.jump_selected = (app.jump_selected + 1) % entries.len();
    }
    if up && !entries.is_empty() {
        app.jump_selected = (app.jump_selected + entries.len() - 1) % entries.len();
    }
    let chosen = if enter && !entries.is_empty() {
        Some(entries[app.jump_selected].id.clone())
    } else {
        None
    };

    let width = ui.available_width();
    let hint = crate::i18n::gettext(app.locale, "Recent chats");
    let search = widgets::search_field(
        ui,
        &palette,
        egui::Id::new("jump-to-chat-query"),
        &mut app.jump_query,
        &hint,
        width,
    );
    if ui.memory(|memory| memory.focused().is_none()) {
        search.request_focus();
    }
    if search.changed() {
        app.jump_selected = 0;
    }

    ui.add_space(6.0);
    if entries.is_empty() {
        theme::text(
            ui,
            crate::i18n::gettext(app.locale, "No open chats match"),
            theme::regular(13.0),
            palette.secondary,
        );
        return;
    }
    let mut clicked = None;
    egui::ScrollArea::vertical()
        .id_salt("jump-to-chat-rows")
        .max_height((ui.ctx().content_rect().height() - 280.0).clamp(120.0, 380.0))
        .auto_shrink([false, true])
        .show(ui, |ui| {
            for (index, entry) in entries.iter().enumerate() {
                let (rect, response) =
                    ui.allocate_exact_size(vec2(ui.available_width(), 40.0), Sense::click());
                if index == app.jump_selected || response.hovered() {
                    widgets::row_highlight(ui, &palette, rect, palette.surface_active);
                }
                if response.hovered() {
                    app.jump_selected = index;
                }
                ui.painter().text(
                    rect.left_center() + vec2(10.0, -8.0),
                    egui::Align2::LEFT_CENTER,
                    &entry.title,
                    theme::regular(14.0),
                    palette.text,
                );
                ui.painter().text(
                    rect.left_center() + vec2(10.0, 9.0),
                    egui::Align2::LEFT_CENTER,
                    &entry.detail,
                    theme::regular(11.0),
                    palette.secondary,
                );
                if index == app.jump_selected {
                    ui.painter().text(
                        rect.right_center() + vec2(-10.0, 0.0),
                        egui::Align2::RIGHT_CENTER,
                        "⏎",
                        theme::regular(13.0),
                        palette.secondary,
                    );
                }
                response.widget_info(|| {
                    egui::WidgetInfo::selected(
                        egui::WidgetType::SelectableLabel,
                        ui.is_enabled(),
                        index == app.jump_selected,
                        format!("{}, {}", entry.title, entry.detail),
                    )
                });
                if response.clicked() {
                    clicked = Some(entry.id.clone());
                }
            }
        });
    if let Some(id) = clicked.or(chosen) {
        execute(app, &id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app() -> (tempfile::TempDir, App) {
        let root = tempfile::tempdir().unwrap();
        let (app, _) = App::headless(
            crate::paths::AppDirs::under(root.path()),
            crate::settings::Settings::default(),
        );
        (root, app)
    }

    fn chat(app: &mut App, id: &str, title: &str, network: &str) {
        let mut chat = crate::model::Chat::new(id.to_string(), title.to_string());
        chat.network = Some(network.to_string());
        app.chats.push(chat);
        app.open_tabs.push(id.to_string());
    }

    #[test]
    fn entries_list_recents_in_open_order_only() {
        let (_root, mut app) = app();
        chat(&mut app, "1@example", "Ada", "WhatsApp");
        chat(&mut app, "2@example", "Boris", "IRC");
        // A chat that is not open stays out.
        app.chats
            .push(crate::model::Chat::new("3@example".into(), "Carol".into()));
        // A stale entry without a chat stays out too.
        app.open_tabs.push("gone@example".into());
        let rows = entries(&app, "");
        let ids: Vec<_> = rows.iter().map(|row| row.id.as_str()).collect();
        assert_eq!(ids, ["1@example", "2@example"]);
        assert_eq!(rows[1].detail, "IRC");
    }

    #[test]
    fn query_matches_titles_case_insensitively() {
        let (_root, mut app) = app();
        chat(&mut app, "1@example", "Ada", "WhatsApp");
        chat(&mut app, "2@example", "Boris", "IRC");
        let rows = entries(&app, "ADA");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, "1@example");
        assert!(entries(&app, "zzz-no-such-chat").is_empty());
    }

    #[test]
    fn the_current_chat_is_marked() {
        let (_root, mut app) = app();
        chat(&mut app, "1@example", "Ada", "WhatsApp");
        chat(&mut app, "2@example", "Boris", "IRC");
        app.open_chat = Some("2@example".into());
        let rows = entries(&app, "");
        assert!(!rows[0].current);
        assert!(rows[1].current);
        assert_eq!(rows[1].detail, "IRC · current");
    }

    #[test]
    fn locked_chats_stay_out_until_the_folder_is_unlocked() {
        let (_root, mut app) = app();
        chat(&mut app, "1@example", "Ada", "WhatsApp");
        chat(&mut app, "2@example", "Secret", "WhatsApp");
        app.chats[1].locked = true;
        assert_eq!(entries(&app, "").len(), 1);
    }

    #[test]
    fn the_highlight_starts_on_the_chat_after_the_current_one() {
        let (_root, mut app) = app();
        chat(&mut app, "1@example", "Ada", "WhatsApp");
        chat(&mut app, "2@example", "Boris", "IRC");
        chat(&mut app, "3@example", "Carol", "WhatsApp");
        let rows = entries(&app, "");
        // No current chat: the first row.
        assert_eq!(default_selected(&app, &rows), 0);
        // Current in the middle: the next chat.
        app.open_chat = Some("2@example".into());
        assert_eq!(default_selected(&app, &rows), 2);
        // Current last: wraps to the first.
        app.open_chat = Some("3@example".into());
        assert_eq!(default_selected(&app, &rows), 0);
        // A single recent chat cannot alternate.
        app.open_tabs = vec!["1@example".into()];
        app.open_chat = Some("1@example".into());
        let rows = entries(&app, "");
        assert_eq!(default_selected(&app, &rows), 0);
    }

    #[test]
    fn execute_opens_the_chat_and_closes_the_dialog() {
        let (_root, mut app) = app();
        chat(&mut app, "1@example", "Ada", "WhatsApp");
        execute(&mut app, &"1@example".to_string());
        assert!(matches!(
            app.actions.as_slice(),
            [Action::OpenChat(id), Action::CloseDialog] if id == "1@example"
        ));
    }
}
