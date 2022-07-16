use super::{edit, AddTextOp, Command, Op};
use crate::syntax::Coords;
use anyhow::Result;
use clap::Args;
use std::path::PathBuf;

/// add an image
#[derive(Args)]
pub struct AddTextCommand {
    /// text content
    text: String,
    /// page number to add the text to
    #[clap(long)]
    page: i32,
    /// font (only standard/builtin fonts supported at this time)
    #[clap(long)]
    font: String,
    /// font size in points
    #[clap(long)]
    font_size: i32,
    /// where to place the text in points - eg. +50+50
    #[clap(long)]
    placement: Coords,
    /// path to a PDF
    pdf: PathBuf,
    /// path to write the resulting PDF
    out: PathBuf,
}

impl Command for AddTextCommand {
    fn execute(self) -> Result<()> {
        let op = Op::AddText(AddTextOp {
            page: self.page,
            text: self.text,
            font: self.font,
            font_size: self.font_size,
            placement: self.placement,
        });
        edit(&[op], &self.pdf, &self.out)
    }
}
