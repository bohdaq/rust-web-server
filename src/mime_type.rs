pub struct MimeType {}


impl MimeType {
    pub(crate) const APPLICATION_OCTET_STREAM: &'static str = "application/octet-stream";
    pub(crate) const VIDEO_MP4: &'static str = "video/mp4";
    pub(crate) const TEXT_PLAIN: &'static str = "text/plain";

    const MP4_SUFFIX: &'static str = ".mp4";
    const TXT_SUFFIX: &'static str = ".txt";

    pub(crate) fn detect_mime_type(request_uri: &str) -> String {

        let is_video_mp4 = request_uri.ends_with(MimeType::MP4_SUFFIX);
        if is_video_mp4 {
            return MimeType::VIDEO_MP4.to_string();
        }

        let is_txt_suffix = request_uri.ends_with(MimeType::TXT_SUFFIX);
        if is_txt_suffix {
            return MimeType::TEXT_PLAIN.to_string();
        }



        return MimeType::APPLICATION_OCTET_STREAM.to_string();
    }

}


