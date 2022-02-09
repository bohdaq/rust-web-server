pub struct MimeType {}


impl MimeType {
    pub(crate) const APPLICATION_OCTET_STREAM: &'static str = "application/octet-stream";
    pub(crate) const VIDEO_MP4: &'static str = "video/mp4";

    pub(crate) fn detect_mime_type(request_uri: &str) -> String {
        const APPLICATION_OCTET_STREAM: &str = "application/octet-stream";
        const VIDEO_MP4: &str = "video/mp4";

        let mp4_suffix = ".mp4";

        let is_video_mp4 = request_uri.ends_with(mp4_suffix);

        if is_video_mp4 {
            return VIDEO_MP4.to_string();
        }

        return APPLICATION_OCTET_STREAM.to_string();
    }

}


