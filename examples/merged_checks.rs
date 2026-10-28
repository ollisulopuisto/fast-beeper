use zapfast::{
    app::App,
    backend::BeeperAccountInfo,
    model::{Chat, ChatFilter},
    paths::AppDirs,
    settings::Settings,
};
fn main() {
    let shortcut = zapfast::ui::merged::latest_reply_shortcut();
    assert_eq!(shortcut.logical_key, egui::Key::R);
    for actual in [
        egui::Modifiers {
            command: true,
            mac_cmd: true,
            shift: true,
            ..Default::default()
        },
        egui::Modifiers {
            command: true,
            ctrl: true,
            shift: true,
            ..Default::default()
        },
    ] {
        assert!(actual.matches_logically(shortcut.modifiers));
    }
    assert!(
        !egui::Modifiers {
            ctrl: true,
            shift: true,
            ..Default::default()
        }
        .matches_logically(shortcut.modifiers)
    );
    let mac_label = shortcut.format(&egui::ModifierNames::NAMES, true);
    let other_label = shortcut.format(&egui::ModifierNames::NAMES, false);
    assert!(
        mac_label.contains("Cmd") || mac_label.contains("Command"),
        "{mac_label}"
    );
    assert!(other_label.contains("Ctrl"), "{other_label}");
    println!(
        "Shortcut semantics/labels: mac={mac_label}; other={other_label}. Synthetic only, no Mac runtime."
    );
    let root = tempfile::tempdir().unwrap();
    let (mut app, _) = App::headless(AppDirs::under(root.path()), Settings::default());
    for (id, network, account, origin) in [
        ("personal", "WhatsApp", "one", "http://127.0.0.1:23373"),
        ("work", "WhatsApp", "two", "http://127.0.0.1:23373"),
        ("signal", "Signal", "one", "http://127.0.0.1:23373"),
        ("other", "WhatsApp", "one", "http://127.0.0.1:8077"),
    ] {
        let mut chat = Chat::new(id.into(), "Shared title".into());
        chat.backend_network = Some(network.into());
        chat.backend_account = Some(account.into());
        chat.backend_origin = Some(origin.into());
        chat.unread = 1;
        app.chats.push(chat);
    }
    assert_eq!(app.beeper_browse_networks(), vec!["Signal", "WhatsApp"]);
    app.beeper_network_filter = Some("WhatsApp".into());
    assert_eq!(app.visible_chats().len(), 3);
    assert_eq!(app.beeper_browse_accounts().len(), 3);
    app.open_chat = Some("work".into());
    app.composer = "work draft must stay in work".into();
    app.select_beeper_scope(Some("WhatsApp".into()), None);
    assert_eq!(app.open_chat.as_deref(), Some("work"));
    app.beeper_account_filter = app.chats[0].backend_settings_key();
    assert_eq!(app.visible_chats()[0].id, "personal");
    assert_eq!(app.visible_chats().len(), 1);
    assert_eq!(app.unread_chats(ChatFilter::Unread), 1);
    app.search = "Shared".into();
    assert_eq!(app.visible_chats().len(), 1);
    let message = zapfast::model::Message {
        id: "hit".into(),
        chat: "work".into(),
        sender: "x".into(),
        sender_name: None,
        from_me: false,
        timestamp: 1,
        history_order: None,
        content: zapfast::model::Content::text("Shared hit"),
        status: zapfast::model::Delivery::Sent,
        delivered_at: None,
        read_at: None,
        quoted: None,
        reactions: vec![],
        edited: false,
        mentions: vec![],
        forwarded: false,
        thumbnail: None,
    };
    app.search_hits.push(message.clone());
    let mut own = message.clone();
    own.chat = "personal".into();
    app.search_hits.push(own);
    let mut unknown = message;
    unknown.chat = "not-in-inventory".into();
    app.search_hits.push(unknown);
    assert_eq!(app.scoped_search_hits().len(), 1);
    assert_eq!(app.scoped_search_hits()[0].chat, "personal");
    assert_eq!(app.open_chat.as_deref(), Some("work"));
    assert_eq!(app.composer, "work draft must stay in work");
    app.beeper_account_filter = None;
    app.beeper_network_filter = Some("Signal".into());
    assert_eq!(app.visible_chats()[0].id, "signal");
    app.beeper_network_filter = None;
    assert_eq!(app.visible_chats().len(), 4);
    app.beeper_accounts.insert(
        "http://127.0.0.1:23373".into(),
        vec![BeeperAccountInfo {
            id: "chatless".into(),
            name: "Empty".into(),
            network: "Telegram".into(),
            status: "disconnected".into(),
        }],
    );
    app.chats[0].pinned = true;
    app.chats[1].pinned = true;
    app.beeper_network_filter = Some("Telegram".into());
    assert_eq!(app.beeper_global_pins().len(), 2);
    assert!(app.visible_chats().is_empty());
    assert_eq!(app.beeper_browse_accounts().len(), 1);
    app.beeper_accounts
        .insert("http://127.0.0.1:23373".into(), vec![]);
    assert_eq!(app.beeper_browse_accounts().len(), 0);
    app.beeper_network_filter = None;
    app.search.clear();
    app.settings.beeper.people.push(zapfast::merged::Person {
        name: "Manual".into(),
        members: vec![
            zapfast::merged::Member::of(&app.chats[0]).unwrap(),
            zapfast::merged::Member::of(&app.chats[2]).unwrap(),
        ],
        primary: None,
    });
    let list = app.visible_chats();
    assert_eq!(
        list.iter()
            .filter(|c| c.id == "personal" || c.id == "signal")
            .count(),
        1
    );
    let preview = app.beeper_person_preview(&app.chats[0]);
    assert_eq!(preview.unread, 2);
    assert!(app.chat_title(&app.chats[0]).contains("Manual"));
    let wire = serde_json::to_string(&app.settings).unwrap();
    let settings: Settings = serde_json::from_str(&wire).unwrap();
    assert_eq!(settings.beeper.people, app.settings.beeper.people);
    let source = app.settings.beeper.people[0].members[0].clone();
    let other = app.settings.beeper.people[0].members[1].clone();
    assert!(zapfast::merged::reply_route(&source, &other, "collision").is_none());
    assert_eq!(
        zapfast::merged::reply_route(&source, &source, "collision")
            .unwrap()
            .id,
        "collision"
    );
    let mut late = app.search_hits[0].clone();
    late.chat = "signal".into();
    late.timestamp = 9999999999;
    late.content = zapfast::model::Content::text("Latest member message");
    app.conversations
        .entry("signal".into())
        .or_default()
        .messages
        .push(late);
    let preview = app.beeper_person_preview(&app.chats[0]);
    assert_eq!(preview.last_activity, 9999999999);
    assert_eq!(preview.last.unwrap().summary, "Latest member message");
    app.chats[2].pinned = true;
    assert_eq!(
        app.beeper_global_pins()
            .iter()
            .filter(|chat| chat.id == "personal" || chat.id == "signal")
            .count(),
        1
    );
    app.settings.beeper.people[0].primary = Some(source.clone());
    let wire = serde_json::to_string(&app.settings).unwrap();
    let restored: Settings = serde_json::from_str(&wire).unwrap();
    assert_eq!(restored.beeper.people[0].primary.as_ref(), Some(&source));
    app.search = "Manual".into();
    let search_rows = app.beeper_display_chats();
    let linked = search_rows
        .iter()
        .find(|c| c.id == "personal" || c.id == "signal")
        .unwrap();
    assert_eq!(linked.last_activity, 9999999999);
    assert_eq!(
        linked.last.as_ref().unwrap().summary,
        "Latest member message"
    );
    assert_eq!(
        search_rows
            .iter()
            .filter(|c| c.id == "personal" || c.id == "signal")
            .count(),
        1
    );
    println!(
        "PASS: real GUI-library linked list identity, preview aggregation, persistence and routing assertions"
    );
}
