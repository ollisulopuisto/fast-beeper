//! Real GUI-library checks for wrapped controls and exact keyboard dispatch.
use zapfast::{app::App,backend::LinkStatus,model::{Chat,ChatFilter},paths::AppDirs,settings::Settings,ui};
fn main(){
 let root=tempfile::tempdir().unwrap();let(mut app,_)=App::headless(AppDirs::under(root.path()),Settings::default());let ctx=egui::Context::default();app.attach(&ctx);app.link=LinkStatus::Connected;
 for i in 0..10 {let mut chat=Chat::new(format!("chat{i}"),format!("Fixture {i}"));chat.backend_network=Some(format!("Network {i}"));chat.backend_account=Some(format!("account{i}"));chat.backend_origin=Some("http://127.0.0.1:8077".into());chat.last_activity=i+1;app.chats.push(chat);}
 let press=|app:&mut App,key,modifiers|{let mut output=ctx.run_ui(egui::RawInput{events:vec![egui::Event::Key{key,physical_key:None,pressed:true,repeat:false,modifiers}],..Default::default()},|ui|ui::keys::handle(app,ui.ctx()));output.textures_delta.clear();};
 press(&mut app,egui::Key::Num1,egui::Modifiers::COMMAND);assert_eq!(app.beeper_network_filter.as_deref(),Some("Network 0"));
 press(&mut app,egui::Key::Num0,egui::Modifiers::COMMAND);assert_eq!(app.beeper_network_filter.as_deref(),Some("Network 9"));
 press(&mut app,egui::Key::Num1,egui::Modifiers::NONE);assert_eq!(app.beeper_network_filter.as_deref(),Some("Network 9"));
 press(&mut app,egui::Key::R,egui::Modifiers::COMMAND|egui::Modifiers::SHIFT);assert!(app.beeper_unreplied);
 press(&mut app,egui::Key::R,egui::Modifiers::COMMAND|egui::Modifiers::SHIFT);assert!(!app.beeper_unreplied);
 app.chats[2].pinned=true;app.actions.clear();
 press(&mut app,egui::Key::Num1,egui::Modifiers::COMMAND|egui::Modifiers::ALT);assert!(app.actions.iter().any(|a|matches!(a,zapfast::model::Action::OpenChat(id) if id=="chat2")));assert_eq!(app.beeper_network_filter.as_deref(),Some("Network 9"));
 app.actions.clear();let list_mod=if cfg!(target_os="macos"){egui::Modifiers{command:true,mac_cmd:true,ctrl:true,..Default::default()}}else{egui::Modifiers::COMMAND|egui::Modifiers::SHIFT};
 press(&mut app,egui::Key::Num1,list_mod);assert!(app.actions.iter().any(|a|matches!(a,zapfast::model::Action::OpenChat(id) if id=="chat9")));
 app.actions.clear();press(&mut app,egui::Key::Num0,list_mod);assert!(!app.actions.iter().any(|a|matches!(a,zapfast::model::Action::OpenChat(_))));
 app.backend.shutdown();let _=ChatFilter::All;println!("PASS: network slots1/10,plain digits unchanged,unreplied toggles");
}
