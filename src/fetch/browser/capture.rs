use chromiumoxide::cdp::browser_protocol::page::CaptureScreenshotFormat;
use chromiumoxide::page::{Page, ScreenshotParams};

use super::navigate::BrowserFetchError;

pub const MAX_SCREENSHOT_WIDTH: u32 = 1280;
pub const MAX_SCREENSHOT_HEIGHT: u32 = 720;
pub const MAX_SCREENSHOT_BYTES: usize = 4 * 1024 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CaptureError {
    Empty,
    NotPng,
    TruncatedHeader,
    InvalidHeader,
    ZeroDimension,
    Dimensions { width: u32, height: u32 },
    TooLarge { bytes: usize, cap: usize },
}

impl std::fmt::Display for CaptureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => write!(f, "screenshot capture is empty"),
            Self::NotPng => write!(f, "screenshot capture is not a PNG"),
            Self::TruncatedHeader => write!(f, "screenshot PNG header is truncated"),
            Self::InvalidHeader => write!(f, "screenshot PNG IHDR header is invalid"),
            Self::ZeroDimension => write!(f, "screenshot PNG has a zero dimension"),
            Self::Dimensions { width, height } => write!(
                f,
                "screenshot dimensions {width}x{height} exceed the limit of {MAX_SCREENSHOT_WIDTH}x{MAX_SCREENSHOT_HEIGHT}"
            ),
            Self::TooLarge { bytes, cap } => {
                write!(f, "screenshot size {bytes} bytes exceeds the {cap}-byte limit")
            }
        }
    }
}

impl std::error::Error for CaptureError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScreenshotCapture {
    png: Vec<u8>,
    width: u32,
    height: u32,
}

impl ScreenshotCapture {
    pub fn try_new(png: Vec<u8>) -> Result<Self, CaptureError> {
        const SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";

        if png.is_empty() {
            return Err(CaptureError::Empty);
        }
        if png.len() > MAX_SCREENSHOT_BYTES {
            return Err(CaptureError::TooLarge {
                bytes: png.len(),
                cap: MAX_SCREENSHOT_BYTES,
            });
        }
        if png.len() < SIGNATURE.len() {
            return Err(CaptureError::TruncatedHeader);
        }
        if &png[..SIGNATURE.len()] != SIGNATURE {
            return Err(CaptureError::NotPng);
        }
        if png.len() < 24 {
            return Err(CaptureError::TruncatedHeader);
        }
        if &png[12..16] != b"IHDR" {
            return Err(CaptureError::InvalidHeader);
        }

        let width = u32::from_be_bytes([png[16], png[17], png[18], png[19]]);
        let height = u32::from_be_bytes([png[20], png[21], png[22], png[23]]);
        if width == 0 || height == 0 {
            return Err(CaptureError::ZeroDimension);
        }
        if width > MAX_SCREENSHOT_WIDTH || height > MAX_SCREENSHOT_HEIGHT {
            return Err(CaptureError::Dimensions { width, height });
        }

        Ok(Self { png, width, height })
    }

    pub fn png(&self) -> &[u8] {
        &self.png
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }
}

pub(crate) async fn capture_viewport(page: &Page) -> Result<ScreenshotCapture, BrowserFetchError> {
    let png = page
        .screenshot(
            ScreenshotParams::builder()
                .format(CaptureScreenshotFormat::Png)
                .full_page(false)
                .capture_beyond_viewport(false)
                .build(),
        )
        .await
        .map_err(|error| BrowserFetchError::CaptureFailed(error.to_string()))?;
    ScreenshotCapture::try_new(png)
        .map_err(|error| BrowserFetchError::CaptureFailed(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(width: u32, height: u32, size: usize) -> Vec<u8> {
        let mut bytes = vec![0; size.max(24)];
        bytes[..8].copy_from_slice(b"\x89PNG\r\n\x1a\n");
        bytes[12..16].copy_from_slice(b"IHDR");
        bytes[16..20].copy_from_slice(&width.to_be_bytes());
        bytes[20..24].copy_from_slice(&height.to_be_bytes());
        bytes
    }

    #[test]
    fn accepts_dimensions_at_and_below_caps() {
        let at = ScreenshotCapture::try_new(png(1280, 720, 24)).unwrap();
        assert_eq!((at.width(), at.height()), (1280, 720));
        let below = ScreenshotCapture::try_new(png(1279, 719, 24)).unwrap();
        assert_eq!((below.width(), below.height()), (1279, 719));
    }

    #[test]
    fn rejects_dimensions_above_caps() {
        assert!(matches!(
            ScreenshotCapture::try_new(png(1281, 720, 24)),
            Err(CaptureError::Dimensions {
                width: 1281,
                height: 720
            })
        ));
        assert!(matches!(
            ScreenshotCapture::try_new(png(1280, 721, 24)),
            Err(CaptureError::Dimensions {
                width: 1280,
                height: 721
            })
        ));
    }

    #[test]
    fn rejects_invalid_png_headers_and_zero_dimensions() {
        assert_eq!(
            ScreenshotCapture::try_new(Vec::new()),
            Err(CaptureError::Empty)
        );
        assert_eq!(
            ScreenshotCapture::try_new(vec![0; 8]),
            Err(CaptureError::NotPng)
        );
        assert_eq!(
            ScreenshotCapture::try_new(b"\x89PNG\r\n\x1a\n".to_vec()),
            Err(CaptureError::TruncatedHeader)
        );
        let mut invalid_header = png(1, 1, 24);
        invalid_header[12..16].copy_from_slice(b"IDAT");
        assert_eq!(
            ScreenshotCapture::try_new(invalid_header),
            Err(CaptureError::InvalidHeader)
        );
        assert_eq!(
            ScreenshotCapture::try_new(png(0, 1, 24)),
            Err(CaptureError::ZeroDimension)
        );
    }

    #[test]
    fn enforces_encoded_byte_cap_at_boundary() {
        let at = ScreenshotCapture::try_new(png(1280, 720, MAX_SCREENSHOT_BYTES)).unwrap();
        assert_eq!(at.png().len(), MAX_SCREENSHOT_BYTES);
        assert!(matches!(
            ScreenshotCapture::try_new(png(1280, 720, MAX_SCREENSHOT_BYTES + 1)),
            Err(CaptureError::TooLarge { bytes, cap })
                if bytes == MAX_SCREENSHOT_BYTES + 1 && cap == MAX_SCREENSHOT_BYTES
        ));
    }
}
