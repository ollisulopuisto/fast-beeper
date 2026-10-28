//! One local person timeline, with exact member routing and no remote merge calls.
use crate::{
    app::App,
    merged::{Member, Person},
    model::{Action, Chat, ChatKind},
};
#[derive(Clone, Default)]
struct RowCache {
    width: f32,
    heights: std::collections::HashMap<(String, String), f32>,
}
#[derive(Clone, Default)]
struct State {
    destination: Option<Member>,
    reply: Option<(Member, String)>,
    explicit: bool,
    search: String,
    source_filter: Option<Member>,
}
/// Command maps to Cmd on macOS and Ctrl elsewhere, matching the rest of the app.
pub fn latest_reply_shortcut() -> egui::KeyboardShortcut {
    egui::KeyboardShortcut::new(
        egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
        egui::Key::R,
    )
}
fn bounded_note(ui: &mut egui::Ui, text: &str, small: bool) {
    let width = (ui.clip_rect().right() - ui.cursor().left())
        .min(ui.available_width())
        .max(20.0);
    let font = if small {
        egui::TextStyle::Small
    } else {
        egui::TextStyle::Body
    }
    .resolve(ui.style());
    let galley = ui
        .painter()
        .layout(text.to_owned(), font, ui.visuals().text_color(), width);
    let (rect, response) = ui.allocate_exact_size(galley.size(), egui::Sense::hover());
    response
        .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, ui.is_enabled(), text));
    ui.painter()
        .galley(rect.min, galley, ui.visuals().text_color());
}

pub fn show(app: &mut App, ui: &mut egui::Ui, chat: &Chat) -> bool {
    if chat.backend_account.is_none() {
        return false;
    }
    if chat.kind != ChatKind::Direct {
        return false;
    }
    if app.chats.iter().filter(|c| c.id == chat.id).count() != 1 {
        return false;
    }
    let current = Member::of(chat);
    let person = app
        .settings
        .beeper
        .people
        .iter()
        .find(|p| p.members.iter().any(|m| Some(m) == current.as_ref()))
        .cloned();
    if person.is_none() {
        ui.horizontal(|ui| {
            ui.menu_button("Link person…", |ui| {
                ui.label("Local link only. Choose the same person's other DM.");
                let choices: Vec<_> = app
                    .chats
                    .iter()
                    .filter(|c| {
                        c.kind == ChatKind::Direct
                            && !c.locked
                            && c.id != chat.id
                            && c.backend_account.is_some()
                            && c.backend_origin == chat.backend_origin
                            && app.chats.iter().filter(|known| known.id == c.id).count() == 1
                    })
                    .cloned()
                    .collect();
                for other in choices {
                    let already = app
                        .settings
                        .beeper
                        .people
                        .iter()
                        .any(|p| p.members.iter().any(|m| m.matches(&other)));
                    if ui
                        .add_enabled(
                            !already,
                            egui::Button::new(format!(
                                "{} · {}",
                                other.name,
                                other.network.as_deref().unwrap_or("Beeper")
                            )),
                        )
                        .clicked()
                        && let (Some(first), Some(second)) = (current.clone(), Member::of(&other))
                    {
                        app.settings.beeper.people.push(Person {
                            name: chat.name.clone(),
                            members: vec![first, second],
                            primary: None,
                        });
                        app.mark_settings_dirty();
                        ui.close();
                    }
                }
            });
        });
        return false;
    }
    let person = person.unwrap();
    let valid: Vec<_> = person
        .members
        .iter()
        .filter(|m| {
            app.chats.iter().any(|c| {
                m.matches(c)
                    && !c.locked
                    && c.kind == ChatKind::Direct
                    && c.backend_origin == chat.backend_origin
                    && app.chats.iter().filter(|known| known.id == c.id).count() == 1
            })
        })
        .cloned()
        .collect();
    if valid.len() < 2 {
        app.ensure_person_loaded(&[]);
        if ui.button("Remove local link").clicked() {
            app.settings.beeper.people.retain(|p| p != &person);
            app.mark_settings_dirty();
        }
        ui.colored_label(egui::Color32::YELLOW,"Linked person unavailable: member identity missing or locked. Open members separately.");
        return true;
    }
    if valid.len() > 8 {
        ui.label("Linked view supports up to 8 exact member chats.");
        return true;
    }
    let ids: Vec<_> = valid.iter().map(|m| m.chat.clone()).collect();
    app.ensure_person_loaded(&ids);
    let key = egui::Id::new(("linked-person-state", &person.members));
    let mut state = ui
        .ctx()
        .data_mut(|d| d.get_temp::<State>(key).unwrap_or_default());
    if state
        .destination
        .as_ref()
        .is_some_and(|m| !valid.contains(m))
    {
        state = State::default();
    }
    if state.destination.is_none() && state.reply.is_none() {
        match person.default_destination(&valid) {
            Ok(primary) => state.destination = primary.cloned(),
            Err(error) => {
                ui.colored_label(egui::Color32::YELLOW, error);
            }
        }
    }

    ui.horizontal(|ui| {
        ui.heading(&person.name);
        ui.label(format!("{} linked chats · local only", valid.len()));
        if ui.button("Unlink").clicked() {
            app.ensure_person_loaded(&[]);
            app.settings.beeper.people.retain(|p| p != &person);
            app.mark_settings_dirty();
            state = State::default();
        }
    });
    bounded_note(
        ui,
        "One timeline. Reply keeps its source. Choosing another destination sends a new message.",
        false,
    );
    bounded_note(
        ui,
        "Text sends only in linked view. Attachments and typing indicators are disabled here until source routing is verified.",
        true,
    );
    if person.members.len() < 8 {
        ui.menu_button("Add another chat…", |ui| {
            let choices: Vec<_> = app
                .chats
                .iter()
                .filter(|c| {
                    c.kind == ChatKind::Direct
                        && !c.locked
                        && c.backend_account.is_some()
                        && c.backend_origin == chat.backend_origin
                        && app.chats.iter().filter(|known| known.id == c.id).count() == 1
                        && !app
                            .settings
                            .beeper
                            .people
                            .iter()
                            .any(|p| p.members.iter().any(|m| m.matches(c)))
                })
                .cloned()
                .collect();
            for other in choices {
                if ui
                    .button(format!(
                        "{} · {}",
                        other.name,
                        other.network.as_deref().unwrap_or("Beeper")
                    ))
                    .clicked()
                    && let Some(member) = Member::of(&other)
                {
                    if let Some(link) = app
                        .settings
                        .beeper
                        .people
                        .iter_mut()
                        .find(|p| **p == person)
                    {
                        link.members.push(member);
                    }
                    app.mark_settings_dirty();
                    ui.close();
                }
            }
        });
    }

    ui.horizontal(|ui| {
        let primary = person
            .primary
            .as_ref()
            .and_then(|m| app.chat(&m.chat).filter(|c| m.matches(c)))
            .map(|c| c.network.as_deref().unwrap_or(&c.name))
            .unwrap_or("No primary selected");
        egui::ComboBox::from_id_salt("person-primary")
            .selected_text(format!("Primary: {primary}"))
            .show_ui(ui, |ui| {
                if ui
                    .selectable_label(person.primary.is_none(), "No primary")
                    .clicked()
                {
                    if let Some(link) = app
                        .settings
                        .beeper
                        .people
                        .iter_mut()
                        .find(|p| **p == person)
                    {
                        link.primary = None;
                    }
                    if !state.explicit && state.reply.is_none() {
                        state.destination = None;
                    }
                    app.mark_settings_dirty();
                }
                for member in &valid {
                    let c = app.chat(&member.chat).unwrap();
                    if ui
                        .selectable_label(
                            person.primary.as_ref() == Some(member),
                            format!(
                                "{} · {} [{}]",
                                c.name,
                                c.network.as_deref().unwrap_or("Beeper"),
                                member.account
                            ),
                        )
                        .clicked()
                    {
                        if let Some(link) = app
                            .settings
                            .beeper
                            .people
                            .iter_mut()
                            .find(|p| **p == person)
                        {
                            link.primary = Some(member.clone());
                        }
                        if !state.explicit && state.reply.is_none() {
                            state.destination = None;
                        }
                        app.mark_settings_dirty();
                    }
                }
            });
    });
    for member in &valid {
        super::conversation::beeper_live_warning(app, ui, &member.chat);
        if let Some(error) = app.beeper_history_errors.get(&member.chat).cloned() {
            ui.horizontal(|ui| {
                ui.colored_label(
                    egui::Color32::YELLOW,
                    format!(
                        "Partial timeline: {} history failed: {error}",
                        member.account
                    ),
                );
                if ui.button("Retry history").clicked() {
                    app.retry_person_history(&member.chat);
                }
            });
        }
    }
    if valid.len() != person.members.len() {
        ui.colored_label(
            egui::Color32::YELLOW,
            "Some linked chats are unavailable or locked; they are excluded.",
        );
    }
    ui.horizontal(|ui| {
        let destination = state
            .destination
            .as_ref()
            .and_then(|m| app.chat(&m.chat))
            .map(|c| c.network.as_deref().unwrap_or(&c.name))
            .unwrap_or("Choose destination");
        egui::ComboBox::from_id_salt("person-destination")
            .selected_text(destination)
            .show_ui(ui, |ui| {
                for member in &valid {
                    let c = app.chat(&member.chat).unwrap();
                    if ui
                        .selectable_label(
                            state.destination.as_ref() == Some(member),
                            format!(
                                "{} · {} [{}]",
                                c.name,
                                c.network.as_deref().unwrap_or("Beeper"),
                                member.account
                            ),
                        )
                        .clicked()
                    {
                        state.explicit = true;
                        state.destination = Some(member.clone());
                        app.reply_to = state.reply.as_ref().and_then(|(source, id)| {
                            crate::merged::reply_route(source, member, id)
                        });
                    }
                }
            });
        if ui.button("Load older from each chat").clicked() {
            for member in &valid {
                app.load_older(&member.chat, true);
            }
        }
    });
    if state.reply.is_none() {
        app.reply_to = None;
    }
    if let Some((source, id)) = &state.reply {
        app.reply_to = state
            .destination
            .as_ref()
            .and_then(|destination| crate::merged::reply_route(source, destination, id));
        if state.destination.as_ref() != Some(source) {
            ui.colored_label(
                egui::Color32::YELLOW,
                "Cross-over: plain new message on selected destination. No reply reference.",
            );
        }
    }
    if let Some(destination) = state
        .destination
        .as_ref()
        .and_then(|m| app.chat(&m.chat))
        .cloned()
    {
        ui.label(format!(
            "Sending via {} · account {}",
            destination.network.as_deref().unwrap_or("Beeper"),
            destination.backend_account.as_deref().unwrap_or("")
        ));
        let had_quote = app.reply_to.is_some();
        let before = app.actions.len();
        super::conversation::composer_with_capabilities(app, ui, &destination, false);
        if had_quote && app.reply_to.is_none() {
            state.reply = None;
            if !state.explicit {
                state.destination = None;
            }
        }
        if app.actions[before..]
            .iter()
            .any(|a| matches!(a, Action::SendText { .. } | Action::SendPending { .. }))
        {
            state.reply = None;
            if !state.explicit {
                state.destination = None;
            }
        }
    } else {
        ui.label("Choose a destination or reply to a message before writing. Nothing is routed automatically.");
    }
    let messages = crate::merged::ordered_refs(
        valid
            .iter()
            .flat_map(|m| {
                app.conversations
                    .get(&m.chat)
                    .into_iter()
                    .flat_map(|c| c.messages.iter())
            })
            .collect(),
    );
    ui.small(format!("{} loaded messages · Each member pages independently; this is not the full remote history.",messages.len()));
    ui.horizontal(|ui| {
        ui.add(
            egui::TextEdit::singleline(&mut state.search)
                .hint_text("Search loaded messages")
                .desired_width(220.0),
        );
        let label = state
            .source_filter
            .as_ref()
            .and_then(|m| app.chat(&m.chat))
            .map(|c| c.network.as_deref().unwrap_or("Beeper"))
            .unwrap_or("All sources");
        egui::ComboBox::from_id_salt("linked-source-filter")
            .selected_text(label)
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut state.source_filter, None, "All sources");
                for member in &valid {
                    let c = app.chat(&member.chat).unwrap();
                    ui.selectable_value(
                        &mut state.source_filter,
                        Some(member.clone()),
                        format!(
                            "{} [{}]",
                            c.network.as_deref().unwrap_or("Beeper"),
                            member.account
                        ),
                    );
                }
            });
        if ui.button("Clear search").clicked() {
            state.search.clear();
            state.source_filter = None;
        }
    });
    let filtered = !state.search.trim().is_empty() || state.source_filter.is_some();
    let visible: Vec<_> = messages
        .into_iter()
        .filter(|message| {
            if state
                .source_filter
                .as_ref()
                .is_some_and(|member| member.chat != message.chat)
            {
                return false;
            }
            if state.search.trim().is_empty() {
                return true;
            }
            let source = app
                .chat(&message.chat)
                .and_then(|c| c.network.as_deref())
                .unwrap_or("Beeper");
            let body = match &message.content {
                crate::model::Content::Text { text, .. }
                | crate::model::Content::Interactive { text, .. } => {
                    std::borrow::Cow::Borrowed(text.as_str())
                }
                other => std::borrow::Cow::Owned(other.full_summary()),
            };
            crate::merged::search_matches(
                &body,
                &state.search,
                source,
                message.sender_name.as_deref().unwrap_or(&message.sender),
            )
        })
        .collect();
    if filtered {
        ui.small(format!(
            "{} matches in loaded history only. Search does not change sending destination.",
            visible.len()
        ));
    }
    let mut selected_reply = None;
    let shortcut = latest_reply_shortcut();
    let shortcut_label = ui.ctx().format_shortcut(&shortcut);
    let keyboard_reply = ui.input_mut(|input| input.consume_shortcut(&shortcut));
    ui.horizontal_wrapped(|ui| {
        if (ui.add_enabled(
            crate::merged::latest_replyable(&visible).is_some(),
            egui::Button::new(format!("Reply to latest match ({shortcut_label})")),
        ).on_hover_text(format!("Keyboard fallback: {shortcut_label}. Uses the latest non-deleted loaded search result and its exact source. Narrow by search/source first.")).clicked() || keyboard_reply)
            && let Some(message) = crate::merged::latest_replyable(&visible)
            && let Some(member) = valid.iter().find(|member| member.chat == message.chat)
        {
            selected_reply = Some((member.clone(), message.id.clone()));
        }
    });
    bounded_note(ui, "Or double-click a message / use its reply menu.", true);
    let remaining = ui.available_height().max(100.0);
    let mut actions = Vec::new();
    let mut refused_action = false;
    let rows_key = key.with("row-heights");
    let mut cache = ui
        .ctx()
        .data_mut(|data| data.remove_temp::<RowCache>(rows_key).unwrap_or_default());
    if (cache.width - ui.available_width()).abs() > 0.5 {
        cache.width = ui.available_width();
        cache.heights.clear();
    }
    // Cap retained layout metadata, not the user's loaded history.
    if cache.heights.len() > 20_000 {
        cache.heights.clear();
    }
    let mut grew_above = 0.0;
    let mut previous_header = None;
    let mut previous_laid_out = false;
    let mut previous_on_screen = false;
    let output = egui::ScrollArea::vertical()
        .id_salt(("person-timeline", &person.members))
        .stick_to_bottom(!filtered)
        .min_scrolled_height(remaining)
        .max_height(remaining)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let viewport = ui.clip_rect();
            if visible.is_empty() {
                ui.label(if filtered {
                    "No matches in loaded history."
                } else {
                    "Loading linked histories…"
                });
            }
            for original_message in visible {
                let day = jiff::Timestamp::from_second(original_message.timestamp)
                    .map(|t| t.to_zoned(jiff::tz::TimeZone::system()).date().to_string())
                    .unwrap_or_default();
                let header = (
                    original_message.chat.clone(),
                    original_message.sender.clone(),
                    day.clone(),
                    original_message.from_me,
                );
                let boundary = crate::merged::header_boundary(
                    previous_header.as_ref().map(
                        |(chat, sender, day, from_me): &(String, String, String, bool)| {
                            (chat.as_str(), sender.as_str(), day.as_str(), *from_me)
                        },
                    ),
                    (&header.0, &header.1, &header.2, header.3),
                );
                previous_header = Some(header);
                let row_key = (original_message.chat.clone(), original_message.id.clone());
                let top = ui.cursor().top();
                let height = cache.heights.get(&row_key).copied().unwrap_or_else(|| {
                    super::conversation::linked_estimated_height(original_message, cache.width)
                });
                if !crate::merged::row_near_view(
                    top,
                    height,
                    viewport.top(),
                    viewport.bottom(),
                    250.0,
                ) {
                    cache.heights.entry(row_key).or_insert(height);
                    ui.add_space(height);
                    previous_laid_out = false;
                    previous_on_screen = false;
                    continue;
                }
                let mut updated;
                let message = if let Some(quote) = &original_message.quoted {
                    if quote
                        .chat
                        .as_deref()
                        .is_none_or(|source| source == original_message.chat)
                    {
                        if let Some(original) = app
                            .conversations
                            .get(&original_message.chat)
                            .and_then(|history| history.message(&quote.id))
                        {
                            updated = original_message.clone();
                            let quote = updated.quoted.as_mut().unwrap();
                            quote.sender = original.sender.clone();
                            quote.sender_name = original.sender_name.clone();
                            quote.summary = original.content.full_summary();
                            quote.labelled = Some(original.content.labelled());
                            quote.mentions = original.mentions.clone();
                            &updated
                        } else {
                            original_message
                        }
                    } else {
                        original_message
                    }
                } else {
                    original_message
                };
                let Some(member) = valid.iter().find(|m| m.chat == message.chat) else {
                    continue;
                };
                let source_chat = app.chat(&message.chat).unwrap().clone();
                let source = source_chat.network.as_deref().unwrap_or("Beeper");
                let on_screen = top >= viewport.top();
                let show_header =
                    boundary || !previous_laid_out || (on_screen && !previous_on_screen);
                ui.push_id((&message.chat, &message.id), |ui| {
                    if show_header {
                        ui.small(format!(
                            "{} · {source} [{}] · {}",
                            day,
                            member.account,
                            if message.from_me {
                                "You"
                            } else {
                                message.sender_name.as_deref().unwrap_or("Sender")
                            }
                        ));
                    }
                    let before = actions.len();
                    super::conversation::linked_bubble(
                        app,
                        ui,
                        &source_chat,
                        message,
                        &mut actions,
                    );
                    let reply = actions.drain(before..).collect::<Vec<_>>();
                    for action in reply {
                        match action {
                            Action::Reply(id) => {
                                selected_reply = Some((member.clone(), id));
                            }
                            Action::Edit(_)
                            | Action::SelectMessage(_)
                            | Action::SweepMessages { .. }
                            | Action::TranslateLocally(_) => refused_action = true,
                            other => actions.push(other),
                        }
                    }
                });
                previous_laid_out = true;
                previous_on_screen = on_screen;
                let measured = (ui.cursor().top() - top).max(1.0);
                if top < viewport.top() {
                    grew_above += measured - height;
                }
                cache.heights.insert(row_key, measured);
            }
            // Keep the last bubble/hover actions above the fixed composer.
            ui.add_space(20.0);
        });
    let bottom = (output.content_size.y - output.inner_rect.height()).max(0.0);
    if grew_above.abs() > 0.5 && output.state.offset.y > 1.0 && output.state.offset.y < bottom - 2.0
    {
        let mut scroll = output.state;
        scroll.offset.y = (scroll.offset.y + grew_above).max(0.0);
        scroll.store(ui.ctx(), output.id);
        ui.ctx()
            .request_discard("linked row height changed above viewport");
    }
    ui.ctx().data_mut(|data| data.insert_temp(rows_key, cache));
    if let Some((member, id)) = selected_reply {
        state.explicit = false;
        state.reply = Some((member.clone(), id.clone()));
        state.destination = Some(member.clone());
        app.reply_to = crate::merged::reply_route(&member, &member, &id);
    }
    if refused_action {
        app.toast(
            "Open this source chat separately for that action; linked routing was not guessed",
        );
    }
    app.actions.extend(actions);
    ui.ctx().data_mut(|d| d.insert_temp(key, state));
    true
}
