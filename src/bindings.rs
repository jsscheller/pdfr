use anyhow::anyhow;
use image::{DynamicImage, EncodableLayout, GenericImageView};
use libjpegturbo_sys as j;
use pdfium_sys as p;
use std::ffi::c_void;
use std::ffi::CString;
use std::io::Write;
use std::ops::Deref;
use std::os::raw::{c_int, c_ulong};
use std::path::Path;
use std::{fmt, mem, ptr, slice};
use utf16string::{LittleEndian, WString};

type Result<T> = std::result::Result<T, PDFiumError>;

#[derive(Debug)]
pub enum PDFiumError {
    Unknown,
    FileError,
    FormatError,
    PasswordError,
    SecurityError,
    PageError,
}

impl fmt::Display for PDFiumError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PDFiumError")
    }
}

impl std::error::Error for PDFiumError {}

pub struct Library;

impl Library {
    pub fn new() -> Self {
        unsafe {
            p::FPDF_InitLibrary();
        }
        Self
    }
}

impl Drop for Library {
    fn drop(&mut self) {
        unsafe {
            p::FPDF_DestroyLibrary();
        }
    }
}

pub struct Document {
    ptr: p::FPDF_DOCUMENT,
}

impl Document {
    pub fn new() -> Result<Self> {
        let ptr = unsafe { p::FPDF_CreateNewDocument() };
        handle_err()?;
        debug_assert!(!ptr.is_null());
        Ok(Self { ptr })
    }

    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let cpath = CString::new(path.as_ref().to_str().unwrap().to_string()).unwrap();
        let ptr = unsafe { p::FPDF_LoadDocument(cpath.as_ptr(), ptr::null()) };
        handle_err()?;
        debug_assert!(!ptr.is_null());
        Ok(Self { ptr })
    }

    pub fn page_count(&self) -> usize {
        unsafe { p::FPDF_GetPageCount(self.ptr) as usize }
    }

    pub fn create_page(&self, pos: usize, width: f64, height: f64) -> Result<Page> {
        let ptr = unsafe { p::FPDFPage_New(self.ptr, pos as i32, width, height) };
        handle_err()?;
        debug_assert!(!ptr.is_null());
        Ok(Page { ptr })
    }

    pub fn load_page(&self, pos: usize) -> Result<Page> {
        let ptr = unsafe { p::FPDF_LoadPage(self.ptr, pos as i32) };
        handle_err()?;
        debug_assert!(!ptr.is_null());
        Ok(Page { ptr })
    }

    pub fn create_image_object(&self) -> Result<ImageObject> {
        let ptr = unsafe { p::FPDFPageObj_NewImageObj(self.ptr) };
        handle_err()?;
        debug_assert!(!ptr.is_null());
        Ok(ImageObject {
            object: Object { ptr },
        })
    }

    pub fn create_text_object(&self, font: &Font, font_size: f32) -> Result<TextObject> {
        let ptr = unsafe { p::FPDFPageObj_CreateTextObj(self.ptr, font.ptr, font_size) };
        handle_err()?;
        debug_assert!(!ptr.is_null());
        Ok(TextObject {
            object: Object { ptr },
        })
    }

    pub fn load_standard_font(&self, font: &str) -> Result<Font> {
        let c_font = CString::new(font).unwrap();
        let ptr = unsafe { p::FPDFText_LoadStandardFont(self.ptr, c_font.as_ptr()) };
        handle_err()?;
        debug_assert!(!ptr.is_null());
        Ok(Font { ptr })
    }

    // TODO: investigate supporting the FPDF_INCREMENTAL, FPDF_NO_INCREMENTAL,
    // and FPDF_REMOVE_SECURITY flags defined in fpdf_save.h. There's not a lot of information
    // on what they actually do, however.
    // Some small info at https://forum.patagames.com/posts/t155-PDF-SaveFlags.
    pub fn save<W: Write + 'static>(&self, writer: &mut W) -> Result<()> {
        let flags = 0;
        let mut pdfium_file_writer = get_pdfium_file_writer_from_writer(writer);
        let ok = unsafe {
            p::FPDF_SaveAsCopy(
                self.ptr,
                pdfium_file_writer.as_fpdf_file_write_mut_ptr(),
                flags,
            )
        };

        if ok == 1 {
            // Pdfium's return value indicated success. Flush the buffer and return the writer
            // back to the caller.
            pdfium_file_writer
                .flush()
                .map_err(|_| PDFiumError::FileError)?;
        }
        return handle_fail(ok);

        /// Returns a wrapped Pdfium `FPDF_FILEWRITE` struct that uses the given writer as an
        /// output source for Pdfium's file writing callback function.
        pub(crate) fn get_pdfium_file_writer_from_writer<W: Write + 'static>(
            writer: &mut W,
        ) -> FpdfFileWriteExt {
            FpdfFileWriteExt {
                version: 1,
                write_block: Some(write_block_from_callback),
                writer,
            }
        }

        trait PdfiumDocumentWriter: Write {
            // A tiny trait that lets us perform type-erasure on the user-provided Rust writer.
            // This means FpdfFileWriteExt does not need to carry a generic parameter, which simplifies
            // callback overloading in the WASM bindings implementation.

            // Additionally, since Pdfium's save operations are synchronous and immediate, we do
            // not need to take ownership of the user-provided Rust writer; a temporary mutable
            // reference is sufficient.
        }

        impl<W: Write> PdfiumDocumentWriter for W {}

        #[repr(C)]
        pub struct FpdfFileWriteExt<'a> {
            // An extension of Pdfium's FPDF_FILEWRITE struct that adds an extra field to carry the
            // user-provided Rust writer.
            version: c_int,
            write_block: Option<
                unsafe extern "C" fn(
                    file_write_ext_ptr: *mut FpdfFileWriteExt,
                    buf: *const c_void,
                    size: c_ulong,
                ) -> c_int,
            >,
            writer: &'a mut dyn PdfiumDocumentWriter, // Type-erased equivalent of <W: Write>
        }

        impl<'a> FpdfFileWriteExt<'a> {
            /// Returns an `FPDF_FILEWRITE` pointer suitable for passing to `FPDF_SaveAsCopy()`
            /// or `FPDF_SaveWithVersion()`.
            #[inline]
            pub(crate) fn as_fpdf_file_write_mut_ptr(&mut self) -> &mut p::FPDF_FILEWRITE {
                unsafe { &mut *(self as *mut FpdfFileWriteExt as *mut p::FPDF_FILEWRITE) }
            }

            /// Flushes the buffer of the underlying Rust writer.
            #[inline]
            pub(crate) fn flush(&mut self) -> std::io::Result<()> {
                self.writer.flush()
            }
        }

        // The callback function invoked by Pdfium.
        extern "C" fn write_block_from_callback(
            file_write_ext_ptr: *mut FpdfFileWriteExt,
            buf: *const c_void,
            size: c_ulong,
        ) -> c_int {
            let result = unsafe {
                match (*file_write_ext_ptr)
                    .writer
                    .write_all(slice::from_raw_parts(buf as *const u8, size as usize))
                {
                    Ok(()) => 1,
                    Err(_) => 0,
                }
            };

            result
        }
    }
}

impl Drop for Document {
    fn drop(&mut self) {
        unsafe { p::FPDF_CloseDocument(self.ptr) }
    }
}

pub struct Page {
    ptr: p::FPDF_PAGE,
}

impl Page {
    pub fn object_count(&self) -> usize {
        unsafe { p::FPDFPage_CountObjects(self.ptr) as usize }
    }

    pub fn load_object(&self, pos: usize) -> Result<Object> {
        let ptr = unsafe { p::FPDFPage_GetObject(self.ptr, pos as i32) };
        handle_err()?;
        debug_assert!(!ptr.is_null());
        Ok(Object { ptr })
    }

    pub fn width(&self) -> f32 {
        unsafe { p::FPDF_GetPageWidthF(self.ptr) }
    }

    pub fn height(&self) -> f32 {
        unsafe { p::FPDF_GetPageHeightF(self.ptr) }
    }

    // Returns one of the following indicating the page rotation:
    //   0 - No rotation.
    //   1 - Rotated 90 degrees clockwise.
    //   2 - Rotated 180 degrees clockwise.
    //   3 - Rotated 270 degrees clockwise.
    pub fn rotation(&self) -> i32 {
        unsafe { p::FPDFPage_GetRotation(self.ptr) }
    }

    pub fn add_image_object(&self, obj: &ImageObject) -> Result<()> {
        unsafe { p::FPDFPage_InsertObject(self.ptr, obj.ptr) };
        handle_err()
    }

    pub fn add_text_object(&self, obj: &TextObject) -> Result<()> {
        unsafe { p::FPDFPage_InsertObject(self.ptr, obj.ptr) };
        handle_err()
    }

    pub fn generate_content(&self) -> Result<()> {
        handle_fail(unsafe { p::FPDFPage_GenerateContent(self.ptr) })
    }
}

impl Drop for Page {
    fn drop(&mut self) {
        unsafe { p::FPDF_ClosePage(self.ptr) };
    }
}

pub struct Object {
    ptr: p::FPDF_PAGEOBJECT,
}

impl Object {
    pub fn transform(&self, a: f64, b: f64, c: f64, d: f64, e: f64, f: f64) -> Result<()> {
        unsafe { p::FPDFPageObj_Transform(self.ptr, a, b, c, d, e, f) };
        handle_err()
    }

    fn type_(&self) -> i32 {
        unsafe { p::FPDFPageObj_GetType(self.ptr) }
    }

    pub fn into_image(self) -> Option<ImageObject> {
        if self.type_() as u32 == p::FPDF_PAGEOBJ_IMAGE {
            Some(ImageObject { object: self })
        } else {
            None
        }
    }
}

pub struct ImageObject {
    object: Object,
}

impl Deref for ImageObject {
    type Target = Object;

    fn deref(&self) -> &Self::Target {
        &self.object
    }
}

impl ImageObject {
    pub fn bitmap(&self, doc: &Document, page: &Page) -> Result<Bitmap> {
        let ptr = unsafe { p::FPDFImageObj_GetRenderedBitmap(doc.ptr, page.ptr, self.ptr) };
        handle_err()?;
        debug_assert!(!ptr.is_null());
        Ok(Bitmap { ptr })
    }

    pub fn set_bitmap(&self, bmp: &Bitmap) -> Result<()> {
        handle_fail(unsafe { p::FPDFImageObj_SetBitmap(ptr::null_mut(), 0, self.ptr, bmp.ptr) })
    }
}

pub struct TextObject {
    object: Object,
}

impl Deref for TextObject {
    type Target = Object;

    fn deref(&self) -> &Self::Target {
        &self.object
    }
}

impl TextObject {
    pub fn set_text(&self, text: &str) -> Result<()> {
        let w_text = to_wide_string(if text.is_empty() { " " } else { text });
        handle_fail(unsafe { p::FPDFText_SetText(self.ptr, w_text.as_ptr() as p::FPDF_WIDESTRING) })
    }
}

pub struct Font {
    ptr: p::FPDF_FONT,
}

pub struct Bitmap {
    ptr: p::FPDF_BITMAP,
}

impl Bitmap {
    pub fn new(width: i32, height: i32, format: u32) -> Result<Self> {
        let ptr =
            unsafe { p::FPDFBitmap_CreateEx(width, height, format as i32, ptr::null_mut(), 0) };
        handle_err()?;
        debug_assert!(!ptr.is_null());
        Ok(Self { ptr })
    }

    pub fn new_with_image(img: DynamicImage) -> Result<Self> {
        let bmp = Self::new(img.width() as i32, img.height() as i32, p::FPDFBitmap_BGRA)?;
        let img = img.into_bgra8();
        bmp.set_buffer(img.as_bytes())?;
        Ok(bmp)
    }

    fn set_buffer(&self, bytes: &[u8]) -> Result<()> {
        if bytes.len() != self.len() {
            return Err(PDFiumError::Unknown);
        }
        self.buffer().copy_from_slice(bytes);
        Ok(())
    }

    pub fn stride(&self) -> i32 {
        unsafe { p::FPDFBitmap_GetStride(self.ptr) }
    }

    pub fn height(&self) -> i32 {
        unsafe { p::FPDFBitmap_GetHeight(self.ptr) }
    }

    pub fn width(&self) -> i32 {
        unsafe { p::FPDFBitmap_GetWidth(self.ptr) }
    }

    pub fn len(&self) -> usize {
        (self.stride() * self.height()) as usize
    }

    pub fn buffer(&self) -> &mut [u8] {
        unsafe {
            let ptr = p::FPDFBitmap_GetBuffer(self.ptr) as *mut u8;
            slice::from_raw_parts_mut(ptr, self.len())
        }
    }

    pub fn format(&self) -> u32 {
        unsafe { p::FPDFBitmap_GetFormat(self.ptr) as u32 }
    }

    pub fn render_page(&self, page: &Page, width: i32, height: i32, rotation: i32) {
        unsafe {
            p::FPDFBitmap_FillRect(self.ptr, 0, 0, width, height, 0xffffff);
            p::FPDF_RenderPageBitmap(self.ptr, page.ptr, 0, 0, width, height, rotation, 0);
        }
    }

    // `libjpegturbo` is about 4x faster than rust's `image`.
    pub fn write_image(&self, path: impl AsRef<Path>, quality: u8) -> anyhow::Result<()> {
        unsafe {
            let file_name = CString::new(path.as_ref().to_str().unwrap().to_string()).unwrap();
            let mode = CString::new("wb").unwrap();
            let fh = libc::fopen(file_name.as_ptr(), mode.as_ptr());

            let mut err = mem::zeroed();
            let mut cinfo: j::jpeg_compress_struct = mem::zeroed();
            cinfo.err = j::jpeg_std_error(&mut err);
            #[cfg(target_arch = "wasm32")]
            let struct_size = mem::size_of::<j::jpeg_compress_struct>() as u32;
            #[cfg(not(target_arch = "wasm32"))]
            let struct_size = mem::size_of::<j::jpeg_compress_struct>() as u64;
            j::jpeg_CreateCompress(
                &mut cinfo,
                80, // Version
                struct_size,
            );
            j::jpeg_stdio_dest(&mut cinfo, fh as *mut j::FILE);

            cinfo.image_width = self.width() as u32;
            cinfo.image_height = self.height() as u32;
            let (color_space, input_components) = match self.format() {
                p::FPDFBitmap_Gray => (j::J_COLOR_SPACE_JCS_GRAYSCALE, 1),
                p::FPDFBitmap_BGR => (j::J_COLOR_SPACE_JCS_EXT_BGR, 3),
                p::FPDFBitmap_BGRx => (j::J_COLOR_SPACE_JCS_EXT_BGRX, 4),
                p::FPDFBitmap_BGRA => (j::J_COLOR_SPACE_JCS_EXT_BGRA, 4),
                _ => {
                    libc::fclose(fh);
                    return Err(anyhow!("unknown image format"));
                }
            };
            cinfo.in_color_space = color_space;
            cinfo.input_components = input_components;
            j::jpeg_set_defaults(&mut cinfo);

            cinfo.dct_method = j::J_DCT_METHOD_JDCT_ISLOW;
            j::jpeg_set_quality(&mut cinfo, quality as i32, 1);

            j::jpeg_start_compress(&mut cinfo, 1);

            let stride = self.stride() as isize;
            let buf = self.buffer().as_mut_ptr();
            while cinfo.next_scanline < cinfo.image_height {
                let offset = cinfo.next_scanline as isize * stride;
                let mut jsamparray = [buf.offset(offset)];
                j::jpeg_write_scanlines(&mut cinfo, jsamparray.as_mut_ptr(), 1);
            }

            j::jpeg_finish_compress(&mut cinfo);
            j::jpeg_destroy_compress(&mut cinfo);
            libc::fclose(fh);
        }
        Ok(())
    }
}

impl Drop for Bitmap {
    fn drop(&mut self) {
        unsafe { p::FPDFBitmap_Destroy(self.ptr) };
    }
}

fn handle_fail(ok: p::FPDF_BOOL) -> Result<()> {
    if ok == 1 {
        Ok(())
    } else {
        Err(handle_err().err().unwrap_or(PDFiumError::Unknown))
    }
}

fn handle_err() -> Result<()> {
    let ret = unsafe { p::FPDF_GetLastError() } as u32;

    match ret {
        p::FPDF_ERR_SUCCESS => Ok(()),
        p::FPDF_ERR_UNKNOWN => Err(PDFiumError::Unknown),
        p::FPDF_ERR_FILE => Err(PDFiumError::FileError),
        p::FPDF_ERR_FORMAT => Err(PDFiumError::FormatError),
        p::FPDF_ERR_PASSWORD => Err(PDFiumError::PasswordError),
        p::FPDF_ERR_SECURITY => Err(PDFiumError::SecurityError),
        p::FPDF_ERR_PAGE => Err(PDFiumError::PageError),
        // The Pdfium documentation says "... if the previous SDK call succeeded, [then] the
        // return value of this function is not defined". On Linux, at least, a return value
        // of FPDF_ERR_SUCCESS seems to be consistently returned; on Windows, however, the
        // return values are indeed unpredictable. See https://github.com/ajrcarey/pdfium-render/issues/24.
        // Therefore, if the return value does not match one of the FPDF_ERR_* constants, we must
        // assume success.
        _ => Ok(()),
    }
}

fn to_wide_string(s: &str) -> Vec<u8> {
    let mut bytes = WString::<LittleEndian>::from(s).into_bytes();

    // Pdfium appears to expect C-style null termination. Since we are dealing with
    // wide (16-bit) characters, we need two bytes of nulls.

    bytes.push(0);
    bytes.push(0);

    bytes
}
