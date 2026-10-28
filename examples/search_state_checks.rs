//! Query-scoped search failure/recovery checks with real App event handling.
use zapfast::{
    app::App,
    backend::{Event, LinkStatus},
    paths::AppDirs,
    settings::Settings,
};
fn main() {
    let root = tempfile::tempdir().unwrap();
    let (mut app, events) = App::headless(AppDirs::under(root.path()), Settings::default());
    let ctx = egui::Context::default();
    app.attach(&ctx);
    app.link = LinkStatus::Connected;
    app.open_chat = Some("work".into());
    app.chat_search = "current".into();
    app.chat_search_pending = true;
    let send = |query: &str, chat: &str| {
        events
            .send(Event::BeeperChatSearchError {
                chat: chat.into(),
                query: query.into(),
                from: None,
                until: None,
                error: "fixture error".into(),
            })
            .unwrap()
    };
    send("old", "work");
    app.background_frame(&ctx);
    assert!(app.chat_search_pending);
    assert!(app.beeper_chat_search_error.is_none());
    send("current", "other");
    app.background_frame(&ctx);
    assert!(app.chat_search_pending);
    assert!(app.beeper_chat_search_error.is_none());
    send("current", "work");
    app.background_frame(&ctx);
    assert!(!app.chat_search_pending);
    assert!(app.beeper_chat_search_error.is_some());
    events
        .send(Event::ChatHits {
            chat: "work".into(),
            query: "current".into(),
            from: None,
            until: None,
            messages: vec![],
            truncated: false,
        })
        .unwrap();
    app.background_frame(&ctx);
    assert!(app.beeper_chat_search_error.is_none());
    app.backend.shutdown();
    println!(
        "PASS: stale query/chat errors ignored; current failure ends pending; success clears error"
    );
}
