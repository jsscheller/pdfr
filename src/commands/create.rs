use super::Command;
use crate::bindings::{Bitmap, Document};
use anyhow::Result;
use clap::Args;
use image::GenericImageView;
use std::fs::File;
use std::path::PathBuf;

/// create a PDF (currently just supports images)
#[derive(Args)]
pub struct CreateCommand {
    /// dots/pixels per inch - used for images
    #[clap(long, default_value_t = 300)]
    dpi: u32,
    /// an image which will be converted to a page in the resulting PDF
    #[clap(long)]
    image: Vec<PathBuf>,
    /// path to write the resulting PDF
    out: PathBuf,
}

impl Command for CreateCommand {
    fn execute(self) -> Result<()> {
        let doc = Document::new()?;

        for (pos, path) in self.image.iter().enumerate() {
            let img = image::io::Reader::open(&path)?.decode()?;
            let width = (img.width() as f64 / self.dpi as f64 * 72.).round();
            let height = (img.height() as f64 / self.dpi as f64 * 72.).round();
            let page = doc.create_page(pos, width, height)?;
            let bmp = Bitmap::new_with_image(img)?;
            let obj = doc.create_image_object()?;
            obj.set_bitmap(&bmp)?;
            obj.transform(width, 0., 0., height, 0., 0.)?;
            page.add_image_object(&obj)?;
            page.generate_content()?;
        }

        let mut f = File::options()
            .write(true)
            .truncate(true)
            .create(true)
            .open(&self.out)?;
        doc.save(&mut f)?;

        Ok(())
    }
}
