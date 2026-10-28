//! Harness stub: only the ColorImage type backend.rs's Event enum names.
//! Real egui is a full GUI library; the Beeper adapter never draws.
#[derive(Clone, Debug, PartialEq)]
pub struct ColorImage {
    pub size: [usize; 2],
    pub pixels: Vec<Color32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Color32(pub u8, pub u8, pub u8, pub u8);
