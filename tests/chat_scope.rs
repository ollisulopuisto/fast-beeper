use solmu::{
    app::App,
    model::{Action, Chat, ChatFilter, ChatKind},
    paths::AppDirs,
    settings::Settings,
};

fn frame(app: &mut App, ctx: &egui::Context) {
    let mut out = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1180.0, 780.0),
            )),
            ..Default::default()
        },
        |ui| {
            app.background_frame(ui.ctx());
            app.frame_ui(ui);
        },
    );
    out.textures_delta.clear();
}
#[test]
fn composed_scope_survives_actions_and_search() {
    let root = tempfile::tempdir().unwrap();
    let (mut app, _) = App::headless(AppDirs::under(root.path()), Settings::default());
    let ctx = egui::Context::default();
    app.attach(&ctx);
    let mut group = Chat::new("group@g.us".into(), "Needle group".into());
    group.kind = ChatKind::Group;
    group.unread = 1;
    let direct = Chat::new("peer@s.whatsapp.net".into(), "Needle peer".into());
    let mut locked = Chat::new("secret@s.whatsapp.net".into(), "Needle secret".into());
    locked.locked = true;
    app.chats = vec![group, direct, locked];
    app.actions.push(Action::SetChatFilter(ChatFilter::Unread));
    app.actions.push(Action::SetChatFilter(ChatFilter::Groups));
    frame(&mut app, &ctx);
    assert_eq!(app.chat_filter, ChatFilter::Unread);
    assert_eq!(app.category_filter, Some(ChatFilter::Groups));
    app.search = "needle".into();
    assert_eq!(app.visible_chats().len(), 1);
    assert_eq!(app.outside_scope_matches(), 1);
    app.clear_chat_scope();
    assert_eq!(app.search, "needle");
    assert_eq!(app.visible_chats().len(), 2);
    assert_eq!(app.outside_scope_matches(), 0);
}

#[test]
fn footer_stays_at_bottom_with_zero_results() {
    let root = tempfile::tempdir().unwrap();
    let (mut app, _) = App::headless(AppDirs::under(root.path()), Settings::default());
    let ctx = egui::Context::default();
    app.attach(&ctx);
    app.page = solmu::model::Page::Chats;
    app.link = solmu::backend::LinkStatus::Connected;
    app.search = "no results".into();
    for _ in 0..3 {
        frame(&mut app, &ctx);
    }
    let rect = ctx
        .data(|d| d.get_temp::<egui::Rect>(egui::Id::new("new-message-footer")))
        .unwrap();
    assert!(rect.top() > 700.0 && rect.bottom() <= 780.0, "{rect:?}");
}
