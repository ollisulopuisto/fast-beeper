//! Targeted real GUI-library overflow geometry and interaction diagnostic.
use zapfast::{app::App, paths::AppDirs, settings::Settings, ui::widgets};
fn frame(ctx: &egui::Context, events: Vec<egui::Event>) -> (egui::Rect, Vec<egui::Rect>) {
    let mut chip = egui::Rect::NOTHING;
    let mut output = ctx.run_ui(egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(220.,400.))), events, ..Default::default()
    }, |ui| { widgets::chip_scroll_row(ui,"test",|ui| { ui.horizontal(|ui| {
        for i in 0..8 {let r = ui.button(format!("Filter {i}")); if i==0 {chip=r.rect;}}
    }); }); });
    output.textures_delta.clear();
    fn bars(shape: &egui::Shape, found: &mut Vec<egui::Rect>) {
        match shape {
            egui::Shape::Vec(shapes) => for shape in shapes {bars(shape,found)},
            egui::Shape::Rect(r) if r.rect.height()<12. && r.rect.width()>12. && r.fill.a()>0 => found.push(r.rect), _ => {}
        }
    }
    let mut found=vec![];for shape in output.shapes {bars(&shape.shape,&mut found)}
    (chip,found)
}
fn main() {
    let root=tempfile::tempdir().unwrap();let(mut app,_)=App::headless(AppDirs::under(root.path()),Settings::default());
    let ctx=egui::Context::default();app.attach(&ctx);
    let(mut chip,mut bars)=frame(&ctx,vec![]);
    for _ in 0..8 {(chip,bars)=frame(&ctx,vec![egui::Event::PointerMoved(chip.center())]);}
    assert!(!bars.is_empty());for bar in &bars {assert!(bar.top()>=chip.bottom(),"bar {bar:?} overlaps chip {chip:?}");}
    let handle=*bars.last().unwrap();let grab=egui::pos2(handle.left()+8.,handle.center().y);let initial=chip.left();
    let button=|pos,pressed|egui::Event::PointerButton{pos,button:egui::PointerButton::Primary,pressed,modifiers:egui::Modifiers::NONE};
    frame(&ctx,vec![egui::Event::PointerMoved(grab),button(grab,true)]);
    for n in 1..=4 {frame(&ctx,vec![egui::Event::PointerMoved(grab+egui::vec2(10.*n as f32,0.))]);}
    let(after,_)=frame(&ctx,vec![button(grab+egui::vec2(40.,0.),false)]);
    assert!(after.left()<initial-1.,"drag did not scroll");
    let pointer=egui::pos2(110.,chip.center().y);
    frame(&ctx,vec![egui::Event::PointerMoved(pointer),egui::Event::MouseWheel{unit:egui::MouseWheelUnit::Point,delta:egui::vec2(0.,-60.),modifiers:egui::Modifiers::NONE,phase:egui::TouchPhase::Move}]);
    let(wheeled,_)=frame(&ctx,vec![egui::Event::PointerMoved(pointer)]);
    assert!(wheeled.left()<after.left()-1.,"wheel did not scroll");
    println!("PASS: scrollbar below chips; drag x {} -> {}; wheel -> {}",initial,after.left(),wheeled.left());
}
