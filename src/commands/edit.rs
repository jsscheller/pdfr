use super::Command;
use crate::bindings::{Bitmap, Document, Page};
use crate::syntax::{Coords, Geometry};
use anyhow::Result;
use clap::Args;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs::{self, File};
use std::path::{Path, PathBuf};

/// apply a series of edits to a PDF file
#[derive(Args)]
pub struct EditCommand {
    /// path to a JSON file containing edit operations
    ops: PathBuf,
    /// path to a PDF
    pdf: PathBuf,
    /// path to write the resulting PDF
    out: PathBuf,
}

#[derive(Deserialize)]
#[serde(tag = "op")]
#[serde(rename_all = "snake_case")]
pub enum Op {
    AddImage(AddImageOp),
    AddText(AddTextOp),
}

#[derive(Deserialize)]
pub struct AddImageOp {
    pub page: i32,
    pub image: PathBuf,
    pub placement: Geometry,
}

#[derive(Deserialize)]
pub struct AddTextOp {
    pub page: i32,
    pub text: String,
    pub font: String,
    pub font_size: i32,
    pub placement: Coords,
}

impl Command for EditCommand {
    fn execute(self) -> Result<()> {
        let json = fs::read_to_string(&self.ops)?;
        let ops: Vec<Op> = serde_json::from_str(&json)?;
        edit(&ops, &self.pdf, &self.out)
    }
}

pub fn edit(ops: &[Op], pdf: &Path, out: &Path) -> Result<()> {
    let doc = Document::load(pdf)?;
    let mut pages = HashMap::new();
    let mut bmps = HashMap::new();

    for op in ops.iter() {
        match op {
            Op::AddText(args) => {
                let page = load_page(&doc, args.page, &mut pages)?;
                // TODO: validate that this is a standard font
                let font = doc.load_standard_font(&args.font)?;
                let obj = doc.create_text_object(&font, args.font_size as f32)?;
                obj.set_text(&args.text)?;
                obj.transform(1., 0.0, 0.0, 1., args.placement.x, args.placement.y)?;
                page.add_text_object(&obj)?;
            }
            Op::AddImage(args) => {
                let page = load_page(&doc, args.page, &mut pages)?;
                if !bmps.contains_key(&args.image) {
                    let img = image::io::Reader::open(&args.image)?.decode()?;
                    bmps.insert(args.image.clone(), Bitmap::new_with_image(img)?);
                }
                let bmp = &bmps[&args.image];
                let obj = doc.create_image_object()?;
                obj.set_bitmap(&bmp)?;
                obj.transform(
                    args.placement.width,
                    0.0,
                    0.0,
                    args.placement.height,
                    args.placement.x,
                    args.placement.y,
                )?;
                page.add_image_object(&obj)?;
            }
        }
    }

    for page in pages.values() {
        page.generate_content()?;
    }

    let mut f = File::options()
        .write(true)
        .truncate(true)
        .create(true)
        .open(out)?;
    doc.save(&mut f)?;

    Ok(())
}

fn load_page<'a>(doc: &Document, num: i32, pages: &'a mut HashMap<i32, Page>) -> Result<&'a Page> {
    if !pages.contains_key(&num) {
        pages.insert(num, doc.load_page((num - 1) as usize)?);
    }
    Ok(&pages[&num])
}
