use super::Command;
use crate::bindings::Document;
use anyhow::Result;
use clap::Args;
use std::path::PathBuf;

/// get number of pages in a PDF
#[derive(Args)]
pub struct PageCountCommand {
    /// path to a PDF
    pdf: PathBuf,
}

impl Command for PageCountCommand {
    fn execute(self) -> Result<()> {
        let doc = Document::load(&self.pdf)?;
        print!("{}", doc.page_count());
        Ok(())
    }
}
