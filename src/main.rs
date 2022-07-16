mod bindings;
mod commands;
mod syntax;

use anyhow::Result;
use clap::{Parser, Subcommand};
use commands::Command;

/// a cli for PDFIUM
#[derive(Parser)]
#[clap(version)]
struct Cli {
    #[clap(subcommand)]
    command: CliCommand,
}

#[derive(Subcommand)]
enum CliCommand {
    Render(commands::RenderCommand),
    PageCount(commands::PageCountCommand),
    AddImage(commands::AddImageCommand),
    AddText(commands::AddTextCommand),
    Edit(commands::EditCommand),
    ExtractImages(commands::ExtractImagesCommand),
    Create(commands::CreateCommand),
}

impl Cli {
    pub fn execute(self) -> Result<()> {
        match self.command {
            CliCommand::Render(c) => c.execute(),
            CliCommand::PageCount(c) => c.execute(),
            CliCommand::AddImage(c) => c.execute(),
            CliCommand::AddText(c) => c.execute(),
            CliCommand::Edit(c) => c.execute(),
            CliCommand::ExtractImages(c) => c.execute(),
            CliCommand::Create(c) => c.execute(),
        }
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let lib = bindings::Library::new();
    cli.execute()?;
    drop(lib);

    Ok(())
}
