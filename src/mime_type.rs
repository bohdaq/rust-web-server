pub struct MimeType {}


impl MimeType {
    pub(crate) const APPLICATION_OCTET_STREAM: &'static str = "application/octet-stream";
    pub(crate) const VIDEO_MP4: &'static str = "video/mp4";
    pub(crate) const TEXT_PLAIN: &'static str = "text/plain";
    pub(crate) const TEXT_CSS: &'static str = "text/css";
    pub(crate) const TEXT_HTML: &'static str = "text/html";
    pub(crate) const TEXT_JAVASCRIPT: &'static str = "text/javascript";
    pub(crate) const IMAGE_APNG: &'static str = "image/apng";
    pub(crate) const IMAGE_AVIF: &'static str = "image/avif";

    const MP4_SUFFIX: &'static str = ".mp4";
    const TXT_SUFFIX: &'static str = ".txt";
    const CSS_SUFFIX: &'static str = ".css";
    const HTML_SUFFIX: &'static str = ".html";
    const JS_SUFFIX: &'static str = ".js";
    const APNG_SUFFIX: &'static str = ".apng";
    const AVIF_SUFFIX: &'static str = ".avif";

    pub(crate) fn detect_mime_type(request_uri: &str) -> String {

        let is_video_mp4 = request_uri.ends_with(MimeType::MP4_SUFFIX);
        if is_video_mp4 {
            return MimeType::VIDEO_MP4.to_string();
        }

        let is_txt_suffix = request_uri.ends_with(MimeType::TXT_SUFFIX);
        if is_txt_suffix {
            return MimeType::TEXT_PLAIN.to_string();
        }

        let is_css_suffix = request_uri.ends_with(MimeType::CSS_SUFFIX);
        if is_css_suffix {
            return MimeType::TEXT_CSS.to_string();
        }

        let is_html_suffix = request_uri.ends_with(MimeType::HTML_SUFFIX);
        if is_html_suffix {
            return MimeType::TEXT_HTML.to_string();
        }

        let is_js_suffix = request_uri.ends_with(MimeType::JS_SUFFIX);
        if is_js_suffix {
            return MimeType::TEXT_JAVASCRIPT.to_string();
        }

        let is_apng_suffix = request_uri.ends_with(MimeType::APNG_SUFFIX);
        if is_apng_suffix {
            return MimeType::IMAGE_APNG.to_string();
        }

        let is_avif_suffix = request_uri.ends_with(MimeType::AVIF_SUFFIX);
        if is_avif_suffix {
            return MimeType::IMAGE_AVIF.to_string();
        }

        return MimeType::APPLICATION_OCTET_STREAM.to_string();
    }

}


