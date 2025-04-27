use lalrpop_util::lalrpop_mod;

pub mod ast;

lalrpop_mod!(pub style, "/style_file/style.rs");
