use crate::MediaError;

pub struct LoadedImage {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

pub fn load_image(path: &str) -> Result<LoadedImage, MediaError> {
    let img = image::open(path)?;
    let rgba = img.to_rgba8();
    let (width, height) = rgba.dimensions();
    Ok(LoadedImage {
        width,
        height,
        rgba: rgba.into_raw(),
    })
}
