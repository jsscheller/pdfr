mod add_image;
mod add_text;
mod create;
mod edit;
mod extract_images;
mod page_count;
mod render;

pub use add_image::*;
pub use add_text::*;
pub use create::*;
pub use edit::*;
pub use extract_images::*;
pub use page_count::*;
pub use render::*;

pub trait Command {
    fn execute(self) -> anyhow::Result<()>;
}
