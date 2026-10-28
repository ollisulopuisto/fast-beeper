//! Local triage persistence and exact confirmation, no live transport.
use zapfast::{
    app::App,
    backend::{Event, LinkStatus},
    model::Chat,
    paths::AppDirs,
    settings::Settings,
};
fn main() {
    let root = tempfile::tempdir().unwrap();
    let (mut app, events) = App::headless(AppDirs::under(root.path()), Settings::default());
    let ctx = egui::Context::default();
    app.attach(&ctx);
    app.link = LinkStatus::Connected;
    let mut chat = Chat::new("chat".into(), "Fixture".into());
    chat.backend_origin = Some("http://127.0.0.1:8077".into());
    chat.backend_account = Some("work".into());
    app.chats.push(chat.clone());
    app.open_chat = Some("chat".into());
    let press = |app: &mut App, mods| {
        let mut output = ctx.run_ui(
            egui::RawInput {
                events: vec![egui::Event::Key {
                    key: egui::Key::L,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: mods,
                }],
                ..Default::default()
            },
            |ui| zapfast::ui::keys::handle(app, ui.ctx()),
        );
        output.textures_delta.clear();
    };
    chat.backend_network = Some("Fixture".into());
    app.chats[0] = chat.clone();
    app.settings.app_lock_hash = Some("fixture-hash".into());
    press(&mut app, egui::Modifiers::COMMAND | egui::Modifiers::SHIFT);
    assert!(app.reply_later_contains(&chat));
    assert!(
        !app.actions
            .iter()
            .any(|a| matches!(a, zapfast::model::Action::LockApp))
    );
    app.actions.clear();
    press(
        &mut app,
        egui::Modifiers::COMMAND | egui::Modifiers::SHIFT | egui::Modifiers::ALT,
    );
    assert!(
        app.actions
            .iter()
            .any(|a| matches!(a, zapfast::model::Action::LockApp))
    );
    app.actions.clear();
    let encoded = serde_json::to_string(&app.settings).unwrap();
    let restored: Settings = serde_json::from_str(&encoded).unwrap();
    assert_eq!(restored.beeper.reply_later, app.settings.beeper.reply_later);
    events
        .send(Event::BeeperSendUncertain {
            chat: "chat".into(),
            text: "fixture".into(),
            message: "400".into(),
        })
        .unwrap();
    app.background_frame(&ctx);
    assert!(app.reply_later_contains(&chat));
    events
        .send(Event::BeeperSendConfirmed {
            origin: "http://other".into(),
            account: "work".into(),
            chat: "chat".into(),
        })
        .unwrap();
    app.background_frame(&ctx);
    assert!(app.reply_later_contains(&chat));
    events
        .send(Event::BeeperSendConfirmed {
            origin: "http://127.0.0.1:8077".into(),
            account: "personal".into(),
            chat: "chat".into(),
        })
        .unwrap();
    app.background_frame(&ctx);
    assert!(app.reply_later_contains(&chat));
    events
        .send(Event::BeeperSendConfirmed {
            origin: "http://127.0.0.1:8077".into(),
            account: "work".into(),
            chat: "chat".into(),
        })
        .unwrap();
    app.background_frame(&ctx);
    assert!(!app.reply_later_contains(&chat));
    let member = zapfast::merged::Member::of(&chat).unwrap();
    let mut other = chat.clone();
    other.id = "linked".into();
    other.backend_account = Some("personal".into());
    let linked = zapfast::merged::Member::of(&other).unwrap();
    app.chats.push(other.clone());
    app.settings.beeper.people.push(zapfast::merged::Person {
        name: "Fixture person".into(),
        members: vec![member, linked.clone()],
        primary: None,
    });
    app.toggle_reply_later();
    assert!(app.reply_later_contains(&other));
    let path = root.path().join("saved-settings.json");
    app.settings.save(&path).unwrap();
    let loaded = Settings::load(&path);
    assert_eq!(loaded.beeper.reply_later, app.settings.beeper.reply_later);
    app.beeper_reply_later_filter = true;
    assert_eq!(app.visible_chats().len(), 1);
    events
        .send(Event::BeeperSendConfirmed {
            origin: linked.origin,
            account: linked.account,
            chat: linked.chat,
        })
        .unwrap();
    app.background_frame(&ctx);
    assert!(!app.reply_later_contains(&chat));
    assert!(!app.reply_later_contains(&other));
    app.chats.clear();
    press(&mut app, egui::Modifiers::COMMAND | egui::Modifiers::SHIFT);
    assert!(
        app.actions
            .iter()
            .any(|a| matches!(a, zapfast::model::Action::LockApp))
    );
    app.backend.shutdown();
    println!(
        "PASS: queue serializes, failed/foreign confirmation preserves, exact confirmation clears"
    );
}
