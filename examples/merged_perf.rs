use zapfast::{
    merged,
    model::{Content, Delivery, Message},
};
fn main() {
    let count = std::env::args()
        .nth(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(10000);
    let source: Vec<_> = (0..count)
        .map(|n| Message {
            id: n.to_string(),
            chat: format!("chat{}", n % 8),
            sender: "friend".into(),
            sender_name: None,
            from_me: false,
            timestamp: (count - n) as i64,
            history_order: None,
            content: Content::text("fixture ".repeat(512)),
            status: Delivery::Sent,
            delivered_at: None,
            read_at: None,
            quoted: None,
            reactions: vec![],
            edited: false,
            mentions: vec![],
            forwarded: false,
            thumbnail: None,
        })
        .collect();
    let start = std::time::Instant::now();
    if std::env::args().nth(1).as_deref() == Some("owned") {
        let rows = merged::ordered(source.clone());
        std::hint::black_box(rows);
    } else {
        let rows = merged::ordered_refs(source.iter().collect());
        std::hint::black_box(rows);
    }
    println!("rows={count} elapsed_us={}", start.elapsed().as_micros());
}
