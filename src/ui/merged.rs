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
    search_open: bool,
}
/// Command maps to Cmd on macOS and Ctrl elsewhere, matching the rest of the app.
pub fn latest_reply_shortcut() -> egui::KeyboardShortcut {
    egui::KeyboardShortcut::new(
        egui::Modifiers::COMMAND | egui::Modifiers::SHIFT | egui::Modifiers::ALT,
        egui::Key::R,
    )
}
fn bounded_note(ui: &mut egui::Ui, palette: &crate::theme::Palette, text: &str, small: bool) {
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
        .layout(text.to_owned(), font, palette.text, width);
    let (rect, response) = ui.allocate_exact_size(galley.size(), egui::Sense::hover());
    response
        .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, ui.is_enabled(), text));
    ui.painter().galley(rect.min, galley, palette.text);
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
        // Linking lives in Contact info; the conversation top stays clean.
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
    let mut state = match ui.ctx().data_mut(|d| d.get_temp::<State>(key)) {
        Some(state) => state,
        None => {
            // Demo captures seed a reply through a real route.
            #[cfg(feature = "demo")]
            {
                let mut state = State::default();
                if crate::demo::linked_reply() {
                    let mut latest: Option<(Member, String, i64)> = None;
                    for member in &valid {
                        if let Some(conversation) = app.conversations.get(&member.chat) {
                            for message in &conversation.messages {
                                if latest
                                    .as_ref()
                                    .is_none_or(|(_, _, ts)| message.timestamp > *ts)
                                {
                                    latest = Some((
                                        member.clone(),
                                        message.id.clone(),
                                        message.timestamp,
                                    ));
                                }
                            }
                        }
                    }
                    if let Some((member, id, _)) = latest {
                        state.reply = Some((member.clone(), id));
                        state.destination = Some(member);
                    }
                }
                state
            }
            #[cfg(not(feature = "demo"))]
            {
                State::default()
            }
        }
    };
    if state
        .destination
        .as_ref()
        .is_some_and(|m| !valid.contains(m))
    {
        state = State::default();
    }
    let mut routing_error = None;
    if state.destination.is_none() && state.reply.is_none() {
        match person.default_destination(&valid) {
            Ok(primary) => state.destination = primary.cloned(),
            Err(error) => routing_error = Some(error),
        }
    }
    let message_count: usize = valid
        .iter()
        .map(|m| {
            app.conversations
                .get(&m.chat)
                .map_or(0, |c| c.messages.len())
        })
        .sum();
    // Identity only: management and the explanatory notes live in the
    // Contact info panel, so the timeline keeps its room.
    let searching =
        state.search_open || !state.search.trim().is_empty() || state.source_filter.is_some();
    // Right-aligned beside the identity while both fit; on their own row
    // below when they would collide, as in the squeezed info layout. The
    // right_to_left block cannot wrap, so measure instead of hoping.
    let tail = format!("{} linked chats · local only", valid.len());
    let actions_width = ["Load older", "Search"]
        .iter()
        .map(|label| {
            ui.fonts(|fonts| {
                fonts
                    .layout_no_wrap(
                        (*label).to_owned(),
                        egui::TextStyle::Button.resolve(ui.style()),
                        egui::Color32::WHITE,
                    )
                    .size()
                    .x
            }) + 2.0 * ui.spacing().button_padding.x
        })
        .sum::<f32>()
        + ui.spacing().item_spacing.x;
    let identity_width = ui.fonts(|fonts| {
        fonts
            .layout_no_wrap(
                person.name.clone(),
                egui::TextStyle::Heading.resolve(ui.style()),
                egui::Color32::WHITE,
            )
            .size()
            .x
            + fonts
                .layout_no_wrap(
                    tail.clone(),
                    egui::TextStyle::Body.resolve(ui.style()),
                    egui::Color32::WHITE,
                )
                .size()
                .x
    }) + 2.0 * ui.spacing().item_spacing.x;
    let actions_inline = identity_width + actions_width <= ui.available_width();
    let mut actions = |ui: &mut egui::Ui, app: &mut App, state: &mut State| {
        if ui
            .button("Load older")
            .on_hover_text(format!(
                "Load older from each chat. {message_count} loaded messages · Each member pages independently; this is not the full remote history."
            ))
            .clicked()
        {
            for member in &valid {
                app.load_older(&member.chat, true);
            }
        }
        if ui
            .selectable_label(searching, "Search")
            .on_hover_text("Search the loaded messages and narrow by source")
            .clicked()
        {
            state.search_open = !searching;
            if !state.search_open {
                state.search.clear();
                state.source_filter = None;
            }
        }
    };
    ui.horizontal(|ui| {
        ui.heading(&person.name);
        ui.label(&tail);
        if actions_inline {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                actions(ui, app, state);
            });
        }
    });
    if !actions_inline {
        ui.horizontal(|ui| actions(ui, app, state));
    }
    if let Some(error) = routing_error {
        ui.colored_label(egui::Color32::YELLOW, error);
    }
    // One compact line per distinct connection problem, sources combined;
    // the full backend text stays on hover instead of stacking per member.
    let mut warnings: Vec<(Vec<&str>, &String)> = Vec::new();
    for member in &valid {
        if let Some(error) = app.beeper_live_errors.get(&member.chat) {
            let source = app.chat(&member.chat).map(network_label).unwrap_or("Chat");
            match warnings.iter_mut().find(|(_, known)| *known == error) {
                Some((sources, _)) if !sources.contains(&source) => sources.push(source),
                Some(_) => {}
                None => warnings.push((vec![source], error)),
            }
        }
    }
    for (sources, error) in warnings {
        ui.add(
            egui::Label::new(
                egui::RichText::new(format!(
                    "{}: live updates unavailable; updates may be incomplete. Retrying automatically.",
                    sources.join(", ")
                ))
                .color(app.conversation_palette().warning),
            )
            .wrap(),
        )
        .on_hover_text(error);
    }
    for member in &valid {
        if let Some(error) = app.beeper_history_errors.get(&member.chat).cloned() {
            let source = source_label(app, &valid, member);
            ui.horizontal(|ui| {
                ui.colored_label(
                    egui::Color32::YELLOW,
                    format!("Partial timeline: {source} history failed: {error}"),
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
    if searching {
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
                .map(network_label)
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
                            format!("{} [{}]", network_label(c), member.account),
                        );
                    }
                });
            if ui.button("Clear search").clicked() {
                state.search.clear();
                state.source_filter = None;
            }
        });
    }
    // Routing safety: no reply target and no chosen destination means no
    // route at all - new text goes nowhere without an explicit choice.
    if state.reply.is_none() {
        app.reply_to = None;
    }
    let cross_over = if let Some((source, id)) = &state.reply {
        app.reply_to = state
            .destination
            .as_ref()
            .and_then(|destination| crate::merged::reply_route(source, destination, id));
        state.destination.as_ref() != Some(source)
    } else {
        false
    };
    // The composer pins to the bottom of the view, with the destination
    // control directly above it. The first bottom panel shown lands lowest.
    let destination = state
        .destination
        .as_ref()
        .and_then(|m| app.chat(&m.chat))
        .cloned();
    if let Some(destination) = &destination {
        let had_quote = app.reply_to.is_some();
        let before = app.actions.len();
        super::conversation::composer_with_capabilities(app, ui, destination, false);
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
        // No route yet: the composer stays visible but inert, and says why.
        egui::Panel::bottom("linked-composer-off")
            .show_separator_line(false)
            .frame(
                egui::Frame::new()
                    .fill(egui::Color32::TRANSPARENT)
                    .inner_margin(egui::Margin {
                        left: 8,
                        right: 8,
                        top: 0,
                        bottom: 8,
                    }),
            )
            .show(ui, |ui| {
                // An inert frame shaped like the input: no disabled fade,
                // no cursor, muted theme-aware text that stays legible in
                // both themes while reading as inactive.
                let palette = app.conversation_palette();
                egui::Frame::new()
                    .fill(palette.bubble_in)
                    .stroke(egui::Stroke::NONE)
                    .corner_radius(egui::CornerRadius::same(24))
                    .inner_margin(egui::Margin {
                        left: 16,
                        right: 16,
                        top: 8,
                        bottom: 8,
                    })
                    .show(ui, |ui| {
                        ui.label(
                            egui::RichText::new(
                                "Choose a destination or reply to a message to write",
                            )
                            .color(palette.secondary),
                        );
                    });
                ui.small("Nothing is routed automatically.");
            });
    }
    egui::Panel::bottom("linked-destination")
        .show_separator_line(false)
        .frame(
            egui::Frame::new()
                .fill(egui::Color32::TRANSPARENT)
                .inner_margin(egui::Margin {
                    left: 8,
                    right: 8,
                    top: 0,
                    bottom: 0,
                }),
        )
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let chosen = destination
                    .as_ref()
                    .map(network_label)
                    .unwrap_or("Choose destination");
                egui::ComboBox::from_id_salt("person-destination")
                    .selected_text(chosen)
                    .show_ui(ui, |ui| {
                        for member in &valid {
                            let c = app.chat(&member.chat).unwrap();
                            if ui
                                .selectable_label(
                                    state.destination.as_ref() == Some(member),
                                    format!("{} · {}", c.name, source_label(app, &valid, member)),
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
                if let Some(destination) = &destination {
                    let via = network_label(destination);
                    let label = account_name(app, destination);
                    ui.small(format!("Sending via {via}{label}"));
                    if cross_over {
                        ui.colored_label(
                            egui::Color32::YELLOW,
                            "Cross-over: plain new message on selected destination. No reply reference.",
                        );
                    }
                }
            });
        });
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
                .map(network_label)
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
    if searching && filtered {
        ui.small(format!(
            "{} matches in loaded history only. Search does not change sending destination.",
            visible.len()
        ));
    }
    let mut selected_reply = None;
    let shortcut = latest_reply_shortcut();
    let shortcut_label = ui.ctx().format_shortcut(&shortcut);
    let keyboard_reply = ui.input_mut(|input| input.consume_shortcut(&shortcut));
    let mut clicked_reply = false;
    if searching {
        ui.horizontal_wrapped(|ui| {
            clicked_reply = ui.add_enabled(
                crate::merged::latest_replyable(&visible).is_some(),
                egui::Button::new(format!("Reply to latest match ({shortcut_label})")),
            ).on_hover_text(format!("Keyboard fallback: {shortcut_label}. Uses the latest non-deleted loaded search result and its exact source. Narrow by search/source first. Or double-click a message / use its reply menu.")).clicked();
        });
    }
    if (clicked_reply || keyboard_reply)
        && let Some(message) = crate::merged::latest_replyable(&visible)
        && let Some(member) = valid.iter().find(|member| member.chat == message.chat)
    {
        selected_reply = Some((member.clone(), message.id.clone()));
    }
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
                let on_screen = top >= viewport.top();
                // Colour is never the only cue: every bubble in a mixed
                // timeline names its source network in text above it. The
                // day still appears only where the day actually changes.
                ui.push_id((&message.chat, &message.id), |ui| {
                    let source = source_label(app, &valid, member);
                    let sender = if message.from_me {
                        "You"
                    } else {
                        message.sender_name.as_deref().unwrap_or("Sender")
                    };
                    if boundary {
                        ui.small(format!("{day} · {source} · {sender}"));
                    } else {
                        ui.small(format!("{source} · {sender}"));
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

/// The account's own name from the Beeper inventory, for "Sending via":
/// " · name" when the inventory knows it, nothing otherwise.
fn account_name(app: &App, chat: &Chat) -> String {
    app.beeper_accounts
        .values()
        .flatten()
        .find(|account| Some(&account.id) == chat.backend_account.as_ref())
        .filter(|account| !account.name.is_empty())
        .map(|account| format!(" · {}", account.name))
        .unwrap_or_default()
}

/// The network a Beeper chat shows: the canonical API name first, then the
/// decorated bridge label, then the generic fallback. Matches the field the
/// bubble tint reads, so colour and text never disagree.
fn network_label(chat: &Chat) -> &str {
    chat.backend_network
        .as_deref()
        .or(chat.network.as_deref())
        .unwrap_or("Beeper")
}

/// How a member reads in the linked view: its network, with the account's
/// own name appended only when another member shares that network.
fn source_label(app: &App, valid: &[Member], member: &Member) -> String {
    let chat = app.chat(&member.chat);
    let network = chat.map(network_label).unwrap_or("Beeper").to_owned();
    let shared = valid.iter().any(|other| {
        other != member && app.chat(&other.chat).map(network_label) == Some(network.as_str())
    });
    if shared {
        if let Some(chat) = chat {
            let label = account_name(app, chat);
            if !label.is_empty() {
                return format!("{network}{label}");
            }
        }
        format!("{network} [{}]", member.account)
    } else {
        network
    }
}

/// Fuzzy name matching for link suggestions: 0 exact, 1 prefix, 2
/// substring, 3 subsequence; None when nothing matches. Case-insensitive.
pub(crate) fn name_score(query: &str, candidate: &str) -> Option<u32> {
    let query = query.trim().to_lowercase();
    let candidate = candidate.trim().to_lowercase();
    if query.is_empty() || candidate.is_empty() {
        return None;
    }
    if candidate == query {
        return Some(0);
    }
    if candidate.starts_with(&query) {
        return Some(1);
    }
    if candidate.contains(&query) {
        return Some(2);
    }
    let mut chars = candidate.chars();
    if query
        .chars()
        .all(|wanted| chars.by_ref().any(|have| have == wanted))
    {
        return Some(3);
    }
    None
}

/// Chats the picker may offer for linking with `chat`: the same backend's
/// unlocked direct chats, each known once, not already linked to anyone.
fn link_candidates(app: &App, chat: &Chat) -> Vec<Chat> {
    app.chats
        .iter()
        .filter(|c| {
            c.kind == ChatKind::Direct
                && !c.locked
                && c.id != chat.id
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
        .collect()
}

#[derive(Clone, Default)]
struct Picker {
    open: bool,
    search: String,
    selected: Vec<String>,
}

/// One checkbox row per candidate, grouped as suggestions first, then a
/// tree by network; the selection survives searching and folding.
fn picker(
    app: &App,
    ui: &mut egui::Ui,
    chat: &Chat,
    base_name: &str,
    state: &mut Picker,
    already: usize,
) -> Vec<Member> {
    let candidates = link_candidates(app, chat);
    if candidates.is_empty() {
        ui.label("No other chats on this Beeper server can be linked.");
        return Vec::new();
    }
    ui.add(
        egui::TextEdit::singleline(&mut state.search)
            .hint_text("Search chats")
            .desired_width(f32::INFINITY),
    );
    let query = state.search.trim().to_lowercase();
    let matches = |c: &&Chat| {
        query.is_empty()
            || c.name.to_lowercase().contains(&query)
            || network_label(c).to_lowercase().contains(&query)
    };
    let row = |ui: &mut egui::Ui, state: &mut Picker, c: &Chat| {
        let account = account_name(app, c);
        let network = network_label(c);
        let mut on = state.selected.contains(&c.id);
        if ui
            .checkbox(&mut on, format!("{} · {network}{account}", c.name))
            .changed()
        {
            if on {
                state.selected.push(c.id.clone());
            } else {
                state.selected.retain(|id| id != &c.id);
            }
        }
    };
    if query.is_empty() {
        let mut suggested: Vec<_> = candidates
            .iter()
            .filter_map(|c| name_score(base_name, &c.name).map(|score| (score, c)))
            .collect();
        suggested.sort_by_key(|(score, _)| *score);
        suggested.truncate(3);
        if !suggested.is_empty() {
            ui.label("Suggested by name:");
            for (_, c) in suggested {
                row(ui, state, c);
            }
            ui.add_space(4.0);
        }
    }
    let mut networks: Vec<_> = candidates
        .iter()
        .filter(matches)
        .map(|c| network_label(c).to_owned())
        .collect();
    networks.sort();
    networks.dedup();
    for network in networks {
        let chats: Vec<_> = candidates
            .iter()
            .filter(|c| network_label(c) == network)
            .filter(matches)
            .collect();
        egui::CollapsingHeader::new(format!("{network} ({})", chats.len()))
            .id_salt(("link-picker-network", &network))
            .default_open(true)
            .show(ui, |ui| {
                for c in chats {
                    row(ui, state, c);
                }
            });
    }
    let room = 8_usize.saturating_sub(already);
    let over = state.selected.len() > room;
    let names: Vec<_> = state
        .selected
        .iter()
        .filter_map(|id| app.chat(id))
        .map(|c| c.name.clone())
        .collect();
    if !state.selected.is_empty() {
        ui.small(format!(
            "{} selected: {}",
            state.selected.len(),
            names.join(", ")
        ));
    }
    if over {
        ui.colored_label(
            egui::Color32::YELLOW,
            format!("A linked person supports up to 8 chats; {room} left."),
        );
    }
    if ui
        .add_enabled(
            !state.selected.is_empty() && !over,
            egui::Button::new("Link selected"),
        )
        .clicked()
    {
        return state
            .selected
            .iter()
            .filter_map(|id| app.chat(id))
            .filter_map(Member::of)
            .collect();
    }
    Vec::new()
}

/// The Contact info section owning person links: the explanation, primary
/// destination, unlinking, and the searchable picker. Returns false when
/// the chat is not a linkable Beeper direct chat.
pub(crate) fn sidebar(app: &mut App, ui: &mut egui::Ui, chat: &Chat) -> bool {
    if chat.backend_account.is_none()
        || chat.kind != ChatKind::Direct
        || app.chats.iter().filter(|c| c.id == chat.id).count() != 1
    {
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
    let palette = app.palette;
    ui.add_space(4.0);
    ui.label(egui::RichText::new("Linked person").strong());
    let key = egui::Id::new(("link-picker", &chat.id));
    let mut state = ui
        .ctx()
        .data_mut(|d| d.get_temp::<Picker>(key))
        .unwrap_or_else(|| {
            #[cfg(feature = "demo")]
            if let Some((search, selected)) = crate::demo::link_picker_seed() {
                return Picker {
                    open: true,
                    search,
                    selected,
                };
            }
            Picker::default()
        });
    if let Some(person) = &person {
        bounded_note(
            ui,
            &palette,
            "One timeline. Reply keeps its source. Choosing another destination sends a new message.",
            false,
        );
        bounded_note(
            ui,
            &palette,
            "Text sends only in linked view. Attachments and typing indicators are disabled here until source routing is verified.",
            true,
        );
        // The primary destination new messages fall back to.
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
        let state_key = egui::Id::new(("linked-person-state", &person.members));
        let mut linked = ui
            .ctx()
            .data_mut(|d| d.get_temp::<State>(state_key).unwrap_or_default());
        let primary = person
            .primary
            .as_ref()
            .and_then(|m| app.chat(&m.chat).filter(|c| m.matches(c)))
            .map(network_label)
            .unwrap_or("No primary selected");
        egui::ComboBox::from_id_salt("person-primary")
            .selected_text(format!("Primary: {primary}"))
            .show_ui(ui, |ui| {
                let mut set = |app: &mut App, primary: Option<Member>| {
                    if let Some(link) = app
                        .settings
                        .beeper
                        .people
                        .iter_mut()
                        .find(|p| **p == *person)
                    {
                        link.primary = primary;
                    }
                    if !linked.explicit && linked.reply.is_none() {
                        linked.destination = None;
                    }
                    app.mark_settings_dirty();
                };
                if ui
                    .selectable_label(person.primary.is_none(), "No primary")
                    .clicked()
                {
                    set(app, None);
                }
                for member in &valid {
                    let c = app.chat(&member.chat).unwrap();
                    if ui
                        .selectable_label(
                            person.primary.as_ref() == Some(member),
                            format!("{} · {}", c.name, source_label(app, &valid, member)),
                        )
                        .clicked()
                    {
                        set(app, Some(member.clone()));
                    }
                }
            });
        ui.ctx().data_mut(|d| d.insert_temp(state_key, linked));
        if ui.button("Unlink").clicked() {
            app.ensure_person_loaded(&[]);
            app.settings.beeper.people.retain(|p| p != person);
            app.mark_settings_dirty();
            ui.ctx().data_mut(|d| d.insert_temp(key, Picker::default()));
            return true;
        }
        let base = person.name.clone();
        let already = person.members.len();
        if ui
            .selectable_label(state.open, "Add chats to this person…")
            .clicked()
        {
            state.open = !state.open;
        }
        if state.open {
            let chosen = picker(app, ui, chat, &base, &mut state, already);
            if !chosen.is_empty() {
                if let Some(link) = app
                    .settings
                    .beeper
                    .people
                    .iter_mut()
                    .find(|p| **p == *person)
                {
                    link.members.extend(chosen);
                }
                app.mark_settings_dirty();
                state = Picker::default();
            }
        }
    } else {
        bounded_note(
            ui,
            &palette,
            "Local link only: combine this chat with the same person's other chats on this Beeper server.",
            true,
        );
        if ui.selectable_label(state.open, "Link person…").clicked() {
            state.open = !state.open;
        }
        if state.open {
            let chosen = picker(app, ui, chat, &chat.name, &mut state, 1);
            if !chosen.is_empty()
                && let Some(first) = current.clone()
            {
                app.settings.beeper.people.push(Person {
                    name: chat.name.clone(),
                    members: std::iter::once(first).chain(chosen).collect(),
                    primary: None,
                });
                app.mark_settings_dirty();
                state = Picker::default();
            }
        }
    };
    // Demo captures show the open picker without a manual scroll.
    #[cfg(feature = "demo")]
    if crate::demo::link_picker_seed().is_some() {
        ui.scroll_to_cursor(Some(egui::Align::BOTTOM));
    }
    ui.ctx().data_mut(|d| d.insert_temp(key, state));
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn member_chat(id: &str, account: &str) -> Chat {
        Chat {
            kind: ChatKind::Direct,
            backend_origin: Some("local".into()),
            backend_account: Some(account.into()),
            ..Chat::new(id.into(), format!("Ada {account}"))
        }
    }

    fn linked_app(
        chats: Vec<Chat>,
        people: Vec<Person>,
    ) -> (tempfile::TempDir, App, egui::Context) {
        let directory = tempfile::tempdir().unwrap();
        let (mut app, _events) = App::headless(
            crate::paths::AppDirs::under(directory.path()),
            crate::settings::Settings::default(),
        );
        app.chats = chats;
        app.settings.beeper.people = people;
        let ctx = egui::Context::default();
        app.attach(&ctx);
        (directory, app, ctx)
    }

    /// One frame of the conversation view's merged-person gate.
    fn handled(app: &mut App, ctx: &egui::Context, chat: &Chat) -> bool {
        let mut handled = false;
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(800.0, 600.0),
                )),
                ..Default::default()
            },
            |ui| handled = show(app, ui, chat),
        );
        output.textures_delta.clear();
        handled
    }

    fn ada_pair() -> (Chat, Chat, Person) {
        let wa = member_chat("ada-wa@s.whatsapp.net", "whatsapp");
        let tg = member_chat("ada-tg@telegram", "telegram");
        let person = Person {
            name: "Ada".into(),
            members: vec![Member::of(&wa).unwrap(), Member::of(&tg).unwrap()],
            primary: None,
        };
        (wa, tg, person)
    }

    #[test]
    fn name_suggestions_rank_exact_prefix_substring_subsequence() {
        assert_eq!(name_score("ada", "Ada"), Some(0));
        assert_eq!(name_score("ada", "Ada Lovelace"), Some(1));
        assert_eq!(name_score("love", "Ada Lovelace"), Some(2));
        assert_eq!(name_score("aal", "Ada Lovelace"), Some(3));
        assert_eq!(name_score("zzz", "Ada Lovelace"), None);
        assert_eq!(name_score("", "Ada"), None);
    }

    #[test]
    fn the_picker_offers_only_linkable_chats() {
        let (wa, tg, person) = ada_pair();
        let group = Chat {
            kind: ChatKind::Group,
            backend_origin: Some("local".into()),
            backend_account: Some("whatsapp".into()),
            ..Chat::new("family@g.us".into(), "Family".into())
        };
        let mut locked = member_chat("locked@s.whatsapp.net", "whatsapp");
        locked.locked = true;
        let other_origin = Chat {
            backend_origin: Some("elsewhere".into()),
            ..member_chat("ada-elsewhere@s.whatsapp.net", "whatsapp")
        };
        let free = member_chat("ada-free@signal", "signal");
        let (_dir, app, _ctx) = linked_app(
            vec![wa.clone(), tg, group, locked, other_origin, free.clone()],
            vec![person],
        );
        let candidates = link_candidates(&app, &wa);
        assert_eq!(
            candidates.iter().map(|c| &c.id).collect::<Vec<_>>(),
            vec![&free.id],
            "groups, locked chats, other origins and linked chats stay out"
        );
    }

    #[test]
    fn a_linked_chat_opens_the_combined_view() {
        let (wa, tg, person) = ada_pair();
        let plain = member_chat("bob@s.whatsapp.net", "whatsapp");
        let (_dir, mut app, ctx) =
            linked_app(vec![wa.clone(), tg.clone(), plain.clone()], vec![person]);
        assert!(
            handled(&mut app, &ctx, &wa),
            "a linked chat takes the combined view"
        );
        assert!(
            handled(&mut app, &ctx, &tg),
            "either member opens the same person"
        );
        assert!(
            !handled(&mut app, &ctx, &plain),
            "an unlinked chat stays a plain conversation"
        );
    }

    #[test]
    fn the_source_label_names_the_canonical_network() {
        let (mut wa, mut tg, person) = ada_pair();
        wa.backend_network = Some("WhatsApp".into());
        tg.backend_network = Some("Telegram".into());
        let (_dir, app, _ctx) = linked_app(vec![wa, tg], vec![person]);
        let valid = app.settings.beeper.people[0].members.clone();
        assert_eq!(source_label(&app, &valid, &valid[0]), "WhatsApp");
        assert_eq!(source_label(&app, &valid, &valid[1]), "Telegram");
    }

    #[test]
    fn a_locked_member_keeps_the_link_with_its_warning() {
        let (wa, mut tg, person) = ada_pair();
        tg.locked = true;
        let (_dir, mut app, ctx) = linked_app(vec![wa.clone(), tg], vec![person]);
        assert!(
            handled(&mut app, &ctx, &wa),
            "the link still owns the row and explains itself instead of splitting"
        );
    }
}
