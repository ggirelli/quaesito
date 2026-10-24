//! The upload limits of the server, checked before a file is sent.

use dioxus::prelude::*;

use crate::protocol::UploadLimits;

/// The server's upload limits, once known.
#[derive(Clone, Copy)]
pub(super) struct Limits(pub(super) Resource<Option<UploadLimits>>);

impl Limits {
    pub(super) fn get(self) -> Option<UploadLimits> {
        (*self.0.read()).flatten()
    }
}

/// An error when `file` is larger than `max` bytes, before spending time uploading it.
pub(super) fn too_large(
    file: &dioxus::html::FileData,
    max: Option<u64>,
    what: &str,
) -> Option<String> {
    let max = max?;
    (file.size() > max).then(|| {
        format!(
            "This {what} is too large (max {} MB).",
            UploadLimits::megabytes(max)
        )
    })
}

/// An error when a picture or video is larger than the server takes.
pub(super) fn media_too_large(file: &dioxus::html::FileData, limits: Limits) -> Option<String> {
    let limits = limits.get();
    if file.content_type().is_some_and(|t| t.starts_with("video/")) {
        too_large(file, limits.map(|l| l.video), "video")
    } else {
        too_large(file, limits.map(|l| l.picture), "picture")
    }
}

/// Upload button title: what pictures and videos the server takes.
pub(super) fn media_limits_title(limits: Limits) -> String {
    match limits.get() {
        Some(l) => format!(
            "Pictures up to {} MB; videos (MP4, WebM, MOV) up to {} MB",
            UploadLimits::megabytes(l.picture),
            UploadLimits::megabytes(l.video),
        ),
        None => "A picture, or a video (MP4, WebM, MOV)".to_owned(),
    }
}

/// Accepted by picture-or-video uploads.
pub(super) const MEDIA_ACCEPT: &str = "image/*,video/mp4,video/webm,video/quicktime";
