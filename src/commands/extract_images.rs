use super::Command;
use crate::bindings::Document;
use anyhow::Result;
use clap::Args;
use std::fs;
use std::path::PathBuf;

/// extract embedded images from a PDF
#[derive(Args)]
pub struct ExtractImagesCommand {
    /// JPEG quality argument
    #[clap(long, default_value_t = 92)]
    quality: u8,
    /// only extract images with a width >= min-width
    #[clap(long, default_value_t = 1)]
    min_width: i32,
    /// only extract images with a height >= min-height
    #[clap(long, default_value_t = 1)]
    min_height: i32,
    /// only extract images with an area >= min-area
    #[clap(long, default_value_t = 1)]
    min_area: i32,
    /// path to a PDF
    pdf: PathBuf,
    /// path to a directory where the images will be written
    out_dir: PathBuf,
}

impl Command for ExtractImagesCommand {
    fn execute(self) -> Result<()> {
        let doc = Document::load(&self.pdf)?;

        fs::create_dir_all(&self.out_dir)?;

        let page_count = doc.page_count();

        let mut image_count = 0;
        for pos in 0..page_count {
            let page = doc.load_page(pos)?;
            let obj_count = page.object_count();
            for obj_pos in 0..obj_count {
                let obj = page.load_object(obj_pos)?;
                if let Some(img_obj) = obj.into_image() {
                    let bmp = img_obj.bitmap(&doc, &page)?;
                    if bmp.height() < self.min_height
                        || bmp.width() < self.min_width
                        || bmp.width() * bmp.height() < self.min_area
                    {
                        continue;
                    }
                    image_count += 1;
                    let image_path = self
                        .out_dir
                        .join(format!(
                            "{}_image_{image_count}",
                            self.pdf.file_stem().unwrap().to_str().unwrap(),
                        ))
                        .with_extension("jpg");
                    bmp.write_image(&image_path, self.quality)?;
                }
            }
        }

        Ok(())
    }
}
