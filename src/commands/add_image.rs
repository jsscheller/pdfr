use super::{edit, AddImageOp, Command, Op};
use crate::syntax::Geometry;
use anyhow::Result;
use clap::Args;
use std::path::PathBuf;

/// add an image
#[derive(Args)]
pub struct AddImageCommand {
    /// page number to add the image to
    #[clap(long)]
    page: i32,
    /// where to place the image in points - eg. 100x100+50+50
    #[clap(long)]
    placement: Geometry,
    /// path to an image file
    image: PathBuf,
    /// path to a PDF
    pdf: PathBuf,
    /// path to write the resulting PDF
    out: PathBuf,
}

impl Command for AddImageCommand {
    fn execute(self) -> Result<()> {
        let op = Op::AddImage(AddImageOp {
            page: self.page,
            image: self.image,
            placement: self.placement,
        });
        edit(&[op], &self.pdf, &self.out)
    }
}
