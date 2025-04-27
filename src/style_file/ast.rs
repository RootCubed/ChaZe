use std::collections::HashMap;

#[derive(Debug)]
pub enum Filter {
    IsNode,
    IsWay,
    IsRelation,
    Match(String, String),
    MatchRole(String),
}

#[derive(Debug)]
pub enum FilterType {
    Keep,
    Remove,
}

#[derive(Debug)]
pub enum FilterExpr {
    And(Box<FilterExpr>, Box<FilterExpr>),
    Or(Box<FilterExpr>, Box<FilterExpr>),
    Not(Box<FilterExpr>),
    Filter(Filter),
}

#[derive(Debug)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Color {
    pub fn new(hex: &str) -> Color {
        let hex = hex.trim_start_matches('#');
        let r = u8::from_str_radix(&hex[0..2], 16).unwrap();
        let g = u8::from_str_radix(&hex[2..4], 16).unwrap();
        let b = u8::from_str_radix(&hex[4..6], 16).unwrap();
        Color { r, g, b }
    }
}

#[derive(Debug)]
pub enum FuncArg {
    String(String),
    Color(Color),
    Float(f64),
    RandomColor,
}

#[derive(Debug, Clone)]
pub struct TextPatch {
    pub offset: Option<(f64, f64)>,
    pub rename: Option<String>,
    pub scale: Option<f64>,
}

impl TextPatch {
    pub fn new() -> TextPatch {
        TextPatch {
            offset: None,
            rename: None,
            scale: None,
        }
    }

    pub fn merge(&self, other: TextPatch) -> TextPatch {
        TextPatch {
            offset: other.offset.or(self.offset),
            rename: other.rename.or(self.rename.clone()),
            scale: other.scale.or(self.scale),
        }
    }
}

#[derive(Debug)]
pub enum Command {
    Take(usize),
    Filter(FilterType, FilterExpr),
    DrawFunc {
        ty: String,
        args: HashMap<String, FuncArg>,
    },
    Sub(Vec<Command>),
    OffsetText {
        key: String,
        offsets: HashMap<String, TextPatch>,
    },
}

#[derive(Debug)]
pub struct Layer {
    pub name: String,
    pub commands: Vec<Command>,
}

#[derive(Debug)]
pub struct Meta {
    pub format: (f64, f64),
    pub dpi: f64,
    pub scale: f64,
    pub center: (f64, f64),
}

impl Meta {
    pub fn width_pixels(&self) -> i32 {
        (self.format.0 / 25.4 * self.dpi) as i32
    }

    pub fn height_pixels(&self) -> i32 {
        (self.format.1 / 25.4 * self.dpi) as i32
    }
}

#[derive(Debug)]
pub struct Style {
    pub meta: Meta,
    pub layers: Vec<Layer>,
}
