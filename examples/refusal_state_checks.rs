//! Unsupported content stays in the current local composer; no transport used.
use zapfast::{app::{App,Pending},backend::{Event,LinkStatus,Refusal,Unsent},model::{Chat,ReplyTarget},paths::AppDirs,settings::Settings};
fn main(){
 let root=tempfile::tempdir().unwrap();let(mut app,events)=App::headless(AppDirs::under(root.path()),Settings::default());let ctx=egui::Context::default();app.attach(&ctx);
 app.link=LinkStatus::Connected;app.chats.push(Chat::new("work".into(),"Fixture".into()));app.open_chat=Some("work".into());
 events.send(Event::SendRefused{chat:"work".into(),quoting:Some(ReplyTarget{id:"quote".into(),chat:None}),unsent:Unsent::Image{width:1,height:1,rgba:vec![1,2,3,255],view_once:false,caption:Some("fixture caption".into())},reason:Refusal::UnsupportedBackend}).unwrap();
 app.background_frame(&ctx);
 assert_eq!(app.composer,"fixture caption");assert_eq!(app.reply_to.as_ref().map(|r|r.id.as_str()),Some("quote"));
 assert!(matches!(&app.pending[0].item,Pending::Picture{rgba,..} if rgba.as_ref()==&vec![1,2,3,255]));
 app.backend.shutdown();
 println!("PASS: unsupported image pixels, caption and quote restored by real App event handler");
}
