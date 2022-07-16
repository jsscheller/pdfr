use super::Command;
use crate::bindings::{Bitmap, Document};
use crate::syntax::{Intervals, Size};
use anyhow::Result;
use clap::Args;
use pdfium_sys as p;
use std::path::PathBuf;
use std::{fs, mem};

/// render PDF pages to JPEG images
#[derive(Args)]
pub struct RenderCommand {
    /// rotation is ignored by default - use this flag to respect rotation
    #[clap(long)]
    rotate: bool,
    /// pages to render
    #[clap(long)]
    pages: Option<Intervals>,
    /// output image size
    #[clap(long)]
    size: Option<Size>,
    /// dots/pixels per inch - only relevant if size is unspecified
    #[clap(long, default_value_t = 300)]
    dpi: u32,
    /// JPEG quality argument
    #[clap(long, default_value_t = 92)]
    quality: u8,
    /// path to a PDF
    pdf: PathBuf,
    /// path to a directory where the images will be written
    out_dir: PathBuf,
}

impl Command for RenderCommand {
    fn execute(self) -> Result<()> {
        let doc = Document::load(&self.pdf)?;

        fs::create_dir_all(&self.out_dir)?;

        let page_count = doc.page_count();
        let pages = if let Some(pages) = self.pages.as_ref() {
            pages.clone()
        } else {
            (1..=page_count).into()
        };

        for pos in pages.iter(page_count) {
            let page = doc.load_page(pos - 1)?;

            let (mut width, mut height) = (page.width(), page.height());
            let rotation = if !self.rotate {
                let rotation = page.rotation();
                if rotation == 1 || rotation == 3 {
                    mem::swap(&mut width, &mut height);
                }
                if rotation > 0 {
                    4 - rotation
                } else {
                    0
                }
            } else {
                0
            };
            let size = self.size.as_ref().map_or(
                {
                    // Width/height are in points.
                    // 72 points per inch.
                    let scaled_width = (width / 72. * self.dpi as f32).round();
                    (scaled_width, (scaled_width / width * height).round())
                },
                |size| {
                    let wh = if size.width.is_some() && size.height.is_some() {
                        (size.width.unwrap(), size.height.unwrap())
                    } else if let Some(size_width) = size.width {
                        (size_width, height / width * size_width)
                    } else if let Some(size_height) = size.height {
                        (width / height * size_height, size_height)
                    } else {
                        (width, height)
                    };
                    (wh.0.round(), wh.1.round())
                },
            );
            let width = size.0 as i32;
            let height = size.1 as i32;
            let bmp = {
                let bmp_size = round_bmp_size(size);
                let bmp_width = bmp_size.0 as i32;
                let bmp_height = bmp_size.1 as i32;
                Bitmap::new(bmp_width, bmp_height, p::FPDFBitmap_BGR)?
            };
            bmp.render_page(&page, width, height, rotation);

            let image_path = self
                .out_dir
                .join(format!(
                    "{}_{}",
                    self.pdf.file_stem().unwrap().to_str().unwrap(),
                    pos
                ))
                .with_extension("jpg");
            bmp.write_image(&image_path, self.quality)?;
        }
        Ok(())
    }
}

// width must be multiple of 4
fn round_bmp_size(size: (f32, f32)) -> (f32, f32) {
    if !div_by_4(size.0) {
        let mut inc = 1.;
        while !div_by_4(size.0 + inc) {
            inc += 1.;
        }
        let ratio = size.1 / size.0;
        (size.0 + inc, ((size.0 + inc) * ratio).round())
    } else {
        size
    }
}

fn div_by_4(n: f32) -> bool {
    let div = n / 4.;
    div.round() == div
}
