#[cfg(test)]
mod tests;

use std::ffi::OsStr;
use std::path::Path;

/// MIME type constants and file-extension detection.
///
/// Use [`MimeType::detect_mime_type`] to get the MIME type from a file path,
/// or reference the constants directly (e.g. [`MimeType::APPLICATION_JSON`]).
pub struct MimeType {}


impl MimeType {
    pub const APPLICATION_OCTET_STREAM: &'static str = "application/octet-stream";
    pub const APPLICATION_ABIWORD: &'static str = "application/x-abiword";
    pub const APPLICATION_VND_AMAZON_EBOOK: &'static str = "application/vnd.amazon.ebook";
    pub const APPLICATION_X_BZIP: &'static str = "application/x-bzip";
    pub const APPLICATION_X_BZIP2: &'static str = "application/x-bzip2";
    pub const APPLICATION_X_CDF: &'static str = "application/x-cdf";
    pub const APPLICATION_X_CSH: &'static str = "application/x-csh";
    pub const APPLICATION_MSWORD: &'static str = "application/msword";
    pub const APPLICATION_VND_OPENXMLFORMATS_OFFICEDOCUMENTS_WORDPROCESSINGIMPL_DOCUMENT: &'static str = "application/vnd.openxmlformats-officedocument.wordprocessingml.document";
    pub const APPLICATION_VND_MS_FONTOBJECT: &'static str = "application/vnd.ms-fontobject";
    pub const APPLICATION_EPUB_ZIP: &'static str = "application/epub+zip";
    pub const APPLICATION_GZIP: &'static str = "application/gzip";
    pub const APPLICATION_JAVA_ARCHIVE: &'static str = "application/java-archive";
    pub const APPLICATION_JSON: &'static str = "application/json";
    pub const APPLICATION_JSONLD: &'static str = "application/ld+json";
    pub const APPLICATION_VND_APPLE_INSTALLER_XML: &'static str = "application/vnd.apple.installer+xml";
    pub const APPLICATION_VND_OASIS_OPENDOCUMENT_PRESENTATION: &'static str = "application/vnd.oasis.opendocument.presentation";
    pub const APPLICATION_VND_OASIS_OPENDOCUMENT_SPREADSHEET: &'static str = "application/vnd.oasis.opendocument.spreadsheet";
    pub const APPLICATION_VND_OASIS_OPENDOCUMENT_TEXT: &'static str = "application/vnd.oasis.opendocument.text";
    pub const APPLICATION_OGG: &'static str = "application/ogg";
    pub const APPLICATION_PDF: &'static str = "application/pdf";
    pub const APPLICATION_X_HTTPD_PHP: &'static str = "application/x-httpd-php";
    pub const APPLICATION_VND_MS_POWERPOINT: &'static str = "application/vnd.ms-powerpoint";
    pub const APPLICATION_VND_OPENXMLFORMATS_OFFICEDOCUMENT_PRESENTATIONML_PRESENTATION: &'static str = "application/vnd.openxmlformats-officedocument.presentationml.presentation";
    pub const APPLICATION_VND_RAR: &'static str = "application/vnd.rar";
    pub const APPLICATION_RTF: &'static str = "application/rtf";
    pub const APPLICATION_X_SH: &'static str = "application/x-sh";
    pub const APPLICATION_X_SHOCKWAVE_FLASH: &'static str = "application/x-shockwave-flash";
    pub const APPLICATION_X_TAR: &'static str = "application/x-tar";
    pub const APPLICATION_VND_VISIO: &'static str = "application/vnd.visio";
    pub const APPLICATION_XHTML_XML: &'static str = "application/xhtml+xml";
    pub const APPLICATION_VND_MS_EXCEL: &'static str = "application/vnd.ms-excel";
    pub const APPLICATION_VND_OPENXMLFORMATS_OFFICEDOCUMENT_SPREADSHEETML_SHEET: &'static str = "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet";
    pub const APPLICATION_XML: &'static str = "application/xml";
    pub const APPLICATION_VND_MOZILLA_XUL_XML: &'static str = "application/vnd.mozilla.xul+xml";
    pub const APPLICATION_ZIP: &'static str = "application/zip";
    pub const APPLICATION_X_7Z_COMPRESSED: &'static str = "application/x-7z-compressed";
    pub const APPLICATION_X_X509_CA_CERT: &'static str = "application/x-x509-ca-cert";
    pub const APPLICATION_WASM: &'static str = "application/wasm";


    pub const TEXT_PLAIN: &'static str = "text/plain";
    pub const TEXT_CSS: &'static str = "text/css";
    pub const TEXT_CSV: &'static str = "text/csv";
    pub const TEXT_HTML: &'static str = "text/html";
    pub const TEXT_JAVASCRIPT: &'static str = "text/javascript";
    pub const TEXT_CALENDAR: &'static str = "text/calendar";


    pub const IMAGE_APNG: &'static str = "image/apng";
    pub const IMAGE_AVIF: &'static str = "image/avif";
    pub const IMAGE_GIF: &'static str = "image/gif";
    pub const IMAGE_JPEG: &'static str = "image/jpeg";
    pub const IMAGE_PNG: &'static str = "image/png";
    pub const IMAGE_SVG: &'static str = "image/svg+xml";
    pub const IMAGE_WEBP: &'static str = "image/webp";
    pub const IMAGE_BMP: &'static str = "image/bmp";
    pub const IMAGE_ICO: &'static str = "image/x-icon";
    pub const IMAGE_TIFF: &'static str = "image/tiff";


    pub const AUDIO_AAC: &'static str = "audio/aac";
    pub const AUDIO_FLAC: &'static str = "audio/flac";
    pub const AUDIO_WAV: &'static str = "audio/wav";
    pub const AUDIO_MP4: &'static str = "audio/mp4";
    pub const AUDIO_OGG: &'static str = "audio/oga";
    pub const AUDIO_MIDI: &'static str = "audio/midi";
    pub const AUDIO_MPEG: &'static str = "audio/mpeg";
    pub const AUDIO_OPUS: &'static str = "audio/opus";
    pub const AUDIO_WEBM: &'static str = "audio/webm";



    pub const VIDEO_3GP: &'static str = "video/3gpp";
    pub const VIDEO_MPEG: &'static str = "video/mpeg";
    pub const VIDEO_MP4: &'static str = "video/mp4";
    pub const VIDEO_OGG: &'static str = "video/ogg";
    pub const VIDEO_QUICKTIME: &'static str = "video/quicktime";
    pub const VIDEO_WEBM: &'static str = "video/webm";
    pub const VIDEO_X_MSVIDEO: &'static str = "video/x-msvideo";
    pub const VIDEO_MP2T: &'static str = "video/mp2t";
    pub const VIDEO_3GPP2: &'static str = "video/3gpp2";

    pub const FONT_OTF: &'static str = "font/otf";
    pub const FONT_TTF: &'static str = "font/ttf";
    pub const FONT_WOFF: &'static str = "font/woff";
    pub const FONT_WOFF2: &'static str = "font/woff2";



    pub const TXT_SUFFIX: &'static str = ".txt";
    pub const CSS_SUFFIX: &'static str = ".css";
    pub const HTML_SUFFIX: &'static str = ".html";
    pub const HTM_SUFFIX: &'static str = ".htm";
    pub const JS_SUFFIX: &'static str = ".js";
    pub const MJS_SUFFIX: &'static str = ".mjs";
    pub const APNG_SUFFIX: &'static str = ".apng";
    pub const AVIF_SUFFIX: &'static str = ".avif";
    pub const GIF_SUFFIX: &'static str = ".gif";
    pub const JPG_SUFFIX: &'static str = ".jpg";
    pub const JPEG_SUFFIX: &'static str = ".jpeg";
    pub const JPE_SUFFIX: &'static str = ".jpe";
    pub const JIF_SUFFIX: &'static str = ".jif";
    pub const JFIF_SUFFIX: &'static str = ".jfif";
    pub const PNG_SUFFIX: &'static str = ".png";
    pub const SVG_SUFFIX: &'static str = ".svg";
    pub const WEBP_SUFFIX: &'static str = ".webp";
    pub const BMP_SUFFIX: &'static str = ".bmp";
    pub const ICO_SUFFIX: &'static str = ".ico";
    pub const CUR_SUFFIX: &'static str = ".cur";
    pub const TIF_SUFFIX: &'static str = ".tif";
    pub const TIFF_SUFFIX: &'static str = ".tiff";
    pub const AAC_SUFFIX: &'static str = ".aac";
    pub const FLAC_SUFFIX: &'static str = ".flac";
    pub const WAV_SUFFIX: &'static str = ".wav";
    pub const M4A_SUFFIX: &'static str = ".m4a";
    pub const N3GP_SUFFIX: &'static str = ".3gp";
    pub const MPG_SUFFIX: &'static str = ".mpg";
    pub const MPEG_SUFFIX: &'static str = ".mpeg";
    pub const MP4_SUFFIX: &'static str = ".mp4";
    pub const M4V_SUFFIX: &'static str = ".m4v";
    pub const M4P_SUFFIX: &'static str = ".m4p";
    pub const OGA_SUFFIX: &'static str = ".oga";
    pub const OGG_SUFFIX: &'static str = ".ogg";
    pub const OGV_SUFFIX: &'static str = ".ogv";
    pub const MOV_SUFFIX: &'static str = ".mov";
    pub const WEBM_SUFFIX: &'static str = ".webm";
    pub const ABW_SUFFIX: &'static str = ".abw";
    pub const AVI_SUFFIX: &'static str = ".avi";
    pub const AZV_SUFFIX: &'static str = ".azw";
    pub const BIN_SUFFIX: &'static str = ".bin";
    pub const BZ_SUFFIX: &'static str = ".bz";
    pub const BZ2_SUFFIX: &'static str = ".bz2";
    pub const CDA_SUFFIX: &'static str = ".cda";
    pub const CSH_SUFFIX: &'static str = ".csh";
    pub const CSV_SUFFIX: &'static str = ".csv";
    pub const DOC_SUFFIX: &'static str = ".doc";
    pub const DOCX_SUFFIX: &'static str = ".docx";
    pub const EOT_SUFFIX: &'static str = ".eot";
    pub const EPUB_SUFFIX: &'static str = ".epub";
    pub const GZ_SUFFIX: &'static str = ".gz";
    pub const ICS_SUFFIX: &'static str = ".ics";
    pub const JAR_SUFFIX: &'static str = ".jar";
    pub const JSON_SUFFIX: &'static str = ".json";
    pub const JSONLD_SUFFIX: &'static str = ".jsonld";
    pub const MIDI_SUFFIX: &'static str = ".midi";
    pub const MID_SUFFIX: &'static str = ".mid";
    pub const MP3_SUFFIX: &'static str = ".mp3";
    pub const MPKG_SUFFIX: &'static str = ".mpkg";
    pub const ODP_SUFFIX: &'static str = ".odp";
    pub const ODS_SUFFIX: &'static str = ".ods";
    pub const ODT_SUFFIX: &'static str = ".odt";
    pub const OGX_SUFFIX: &'static str = ".ogx";
    pub const OPUS_SUFFIX: &'static str = ".opus";
    pub const OTF_SUFFIX: &'static str = ".otf";
    pub const PDF_SUFFIX: &'static str = ".pdf";
    pub const PHP_SUFFIX: &'static str = ".php";
    pub const PPT_SUFFIX: &'static str = ".ppt";
    pub const PPTX_SUFFIX: &'static str = ".pptx";
    pub const RAR_SUFFIX: &'static str = ".rar";
    pub const RTF_SUFFIX: &'static str = ".rtf";
    pub const SH_SUFFIX: &'static str = ".sh";
    pub const SWF_SUFFIX: &'static str = ".swf";
    pub const TAR_SUFFIX: &'static str = ".tar";
    pub const TS_SUFFIX: &'static str = ".ts";
    pub const TTF_SUFFIX: &'static str = ".ttf";
    pub const VSD_SUFFIX: &'static str = ".vsd";
    pub const WEBA_SUFFIX: &'static str = ".weba";
    pub const WOFF_SUFFIX: &'static str = ".woff";
    pub const WOFF2_SUFFIX: &'static str = ".woff2";
    pub const XHTML_SUFFIX: &'static str = ".xhtml";
    pub const XLS_SUFFIX: &'static str = ".xls";
    pub const XLSX_SUFFIX: &'static str = ".xlsx";
    pub const XML_SUFFIX: &'static str = ".xml";
    pub const XUL_SUFFIX: &'static str = ".xul";
    pub const ZIP_SUFFIX: &'static str = ".zip";
    pub const N7Z_SUFFIX: &'static str = ".7z";
    pub const N3G2_SUFFIX: &'static str = ".3g2";
    pub const CRT_SUFFIX: &'static str = ".crt";
    pub const WASM_SUFFIX: &'static str = ".wasm";

    /// Returns the MIME type string for the given file path based on its extension.
    /// Falls back to `"application/octet-stream"` for unknown extensions.
    pub fn detect_mime_type(request_uri: &str) -> String {
        MimeType::mime_type_for(request_uri).to_string()
    }

    /// [`MimeType::detect_mime_type`] without allocating: one `match` on the
    /// extension for the usual shape (a last path segment with a dot after
    /// its first character); other shapes — dotfiles, a trailing `/` — take
    /// the original suffix-by-suffix walk, whose `ends_with` and
    /// `Path::extension` checks disagree there. Case-sensitive, like that walk.
    pub(crate) fn mime_type_for(path: &str) -> &'static str {
        let name_start = path
            .rfind(|c: char| c == '/' || (cfg!(windows) && c == '\\'))
            .map_or(0, |i| i + 1);
        let name = &path[name_start..];
        match name.rfind('.') {
            Some(dot) if dot > 0 => MimeType::mime_type_for_extension(&name[dot + 1..]),
            _ => MimeType::mime_type_by_suffix(path),
        }
    }

    /// The MIME type for an extension (without the dot), as the suffix walk
    /// would pick it; `application/octet-stream` for an unknown one.
    fn mime_type_for_extension(extension: &str) -> &'static str {
        match extension {
            "txt" => MimeType::TEXT_PLAIN,
            "css" => MimeType::TEXT_CSS,
            "html" | "htm" => MimeType::TEXT_HTML,
            "mjs" | "js" => MimeType::TEXT_JAVASCRIPT,
            "apng" => MimeType::IMAGE_APNG,
            "avif" => MimeType::IMAGE_AVIF,
            "gif" => MimeType::IMAGE_GIF,
            "svg" => MimeType::IMAGE_SVG,
            "jpg" | "jpeg" | "jpe" | "jif" | "jfif" => MimeType::IMAGE_JPEG,
            "png" => MimeType::IMAGE_PNG,
            "webp" => MimeType::IMAGE_WEBP,
            "bmp" => MimeType::IMAGE_BMP,
            "ico" | "cur" => MimeType::IMAGE_ICO,
            "tif" | "tiff" => MimeType::IMAGE_TIFF,
            "aac" => MimeType::AUDIO_AAC,
            "flac" => MimeType::AUDIO_FLAC,
            "wav" => MimeType::AUDIO_WAV,
            "m4a" => MimeType::AUDIO_MP4,
            "oga" => MimeType::AUDIO_OGG,
            "3gp" => MimeType::VIDEO_3GP,
            "mpg" | "mpeg" => MimeType::VIDEO_MPEG,
            "mp4" | "m4v" | "m4p" => MimeType::VIDEO_MP4,
            "ogg" | "ogv" => MimeType::VIDEO_OGG,
            "mov" => MimeType::VIDEO_QUICKTIME,
            "webm" => MimeType::VIDEO_WEBM,
            "abw" => MimeType::APPLICATION_ABIWORD,
            "avi" => MimeType::VIDEO_X_MSVIDEO,
            "azw" => MimeType::APPLICATION_VND_AMAZON_EBOOK,
            "bz" => MimeType::APPLICATION_X_BZIP,
            "bz2" => MimeType::APPLICATION_X_BZIP2,
            "cda" => MimeType::APPLICATION_X_CDF,
            "csh" => MimeType::APPLICATION_X_CSH,
            "csv" => MimeType::TEXT_CSV,
            "doc" => MimeType::APPLICATION_MSWORD,
            "docx" => MimeType::APPLICATION_VND_OPENXMLFORMATS_OFFICEDOCUMENTS_WORDPROCESSINGIMPL_DOCUMENT,
            "eot" => MimeType::APPLICATION_VND_MS_FONTOBJECT,
            "epub" => MimeType::APPLICATION_EPUB_ZIP,
            "gz" => MimeType::APPLICATION_GZIP,
            "ics" => MimeType::TEXT_CALENDAR,
            "jar" => MimeType::APPLICATION_JAVA_ARCHIVE,
            "json" => MimeType::APPLICATION_JSON,
            "jsonld" => MimeType::APPLICATION_JSONLD,
            "midi" | "mid" => MimeType::AUDIO_MIDI,
            "mp3" => MimeType::AUDIO_MPEG,
            "mpkg" => MimeType::APPLICATION_VND_APPLE_INSTALLER_XML,
            "odp" => MimeType::APPLICATION_VND_OASIS_OPENDOCUMENT_PRESENTATION,
            "ods" => MimeType::APPLICATION_VND_OASIS_OPENDOCUMENT_SPREADSHEET,
            "odt" => MimeType::APPLICATION_VND_OASIS_OPENDOCUMENT_TEXT,
            "ogx" => MimeType::APPLICATION_OGG,
            "opus" => MimeType::AUDIO_OPUS,
            "otf" => MimeType::FONT_OTF,
            "pdf" => MimeType::APPLICATION_PDF,
            "php" => MimeType::APPLICATION_X_HTTPD_PHP,
            "ppt" => MimeType::APPLICATION_VND_MS_POWERPOINT,
            "pptx" => MimeType::APPLICATION_VND_OPENXMLFORMATS_OFFICEDOCUMENT_PRESENTATIONML_PRESENTATION,
            "rar" => MimeType::APPLICATION_VND_RAR,
            "rtf" => MimeType::APPLICATION_RTF,
            "sh" => MimeType::APPLICATION_X_SH,
            "swf" => MimeType::APPLICATION_X_SHOCKWAVE_FLASH,
            "tar" => MimeType::APPLICATION_X_TAR,
            "ts" => MimeType::VIDEO_MP2T,
            "ttf" => MimeType::FONT_TTF,
            "vsd" => MimeType::APPLICATION_VND_VISIO,
            "weba" => MimeType::AUDIO_WEBM,
            "woff" => MimeType::FONT_WOFF,
            "woff2" => MimeType::FONT_WOFF2,
            "xhtml" => MimeType::APPLICATION_XHTML_XML,
            "xls" => MimeType::APPLICATION_VND_MS_EXCEL,
            "xlsx" => MimeType::APPLICATION_VND_OPENXMLFORMATS_OFFICEDOCUMENT_SPREADSHEETML_SHEET,
            "xml" => MimeType::APPLICATION_XML,
            "xul" => MimeType::APPLICATION_VND_MOZILLA_XUL_XML,
            "zip" => MimeType::APPLICATION_ZIP,
            "7z" => MimeType::APPLICATION_X_7Z_COMPRESSED,
            "3g2" => MimeType::VIDEO_3GPP2,
            "crt" => MimeType::APPLICATION_X_X509_CA_CERT,
            "wasm" => MimeType::APPLICATION_WASM,
            _ => MimeType::APPLICATION_OCTET_STREAM,
        }
    }

    /// The original detection: each known suffix in turn, some by
    /// `ends_with` on the whole path, some by `Path::extension`.
    fn mime_type_by_suffix(request_uri: &str) -> &'static str {

        let is_txt_suffix = request_uri.ends_with(MimeType::TXT_SUFFIX);
        if is_txt_suffix {
            return MimeType::TEXT_PLAIN;
        }

        let is_css_suffix = request_uri.ends_with(MimeType::CSS_SUFFIX);
        if is_css_suffix {
            return MimeType::TEXT_CSS;
        }

        let mut is_html_suffix = false;
        let boxed_extension = MimeType::get_extension_from_filename(request_uri);
        if !boxed_extension.is_none() {
            let html_suffixes = vec![MimeType::HTML_SUFFIX, MimeType::HTM_SUFFIX];
            let extension = boxed_extension.unwrap();
            let suffix = [".", extension].join("");
            is_html_suffix = html_suffixes.contains(&suffix.as_str())
        }

        if is_html_suffix {
            return MimeType::TEXT_HTML;
        }

        let mut is_js_suffix = false;
        let boxed_extension = MimeType::get_extension_from_filename(request_uri);
        if !boxed_extension.is_none() {
            let js_suffixes = vec![MimeType::MJS_SUFFIX, MimeType::JS_SUFFIX];
            let extension = boxed_extension.unwrap();
            let suffix = [".", extension].join("");
            is_js_suffix = js_suffixes.contains(&suffix.as_str())
        }

        if is_js_suffix {
            return MimeType::TEXT_JAVASCRIPT;
        }

        let is_apng_suffix = request_uri.ends_with(MimeType::APNG_SUFFIX);
        if is_apng_suffix {
            return MimeType::IMAGE_APNG;
        }

        let is_avif_suffix = request_uri.ends_with(MimeType::AVIF_SUFFIX);
        if is_avif_suffix {
            return MimeType::IMAGE_AVIF;
        }

        let is_gif_suffix = request_uri.ends_with(MimeType::GIF_SUFFIX);
        if is_gif_suffix {
            return MimeType::IMAGE_GIF;
        }

        let is_svg_suffix = request_uri.ends_with(MimeType::SVG_SUFFIX);
        if is_svg_suffix {
            return MimeType::IMAGE_SVG;
        }

        let mut is_jpeg_suffix = false;
        let boxed_extension = MimeType::get_extension_from_filename(request_uri);
        if !boxed_extension.is_none() {
            let jpeg_suffixes = vec![MimeType::JPG_SUFFIX, MimeType::JPEG_SUFFIX, MimeType::JPE_SUFFIX, MimeType::JIF_SUFFIX, MimeType::JFIF_SUFFIX];
            let extension = boxed_extension.unwrap();
            let suffix = [".", extension].join("");
            is_jpeg_suffix = jpeg_suffixes.contains(&suffix.as_str())
        }

        if is_jpeg_suffix {
            return MimeType::IMAGE_JPEG;
        }

        let is_png_suffix = request_uri.ends_with(MimeType::PNG_SUFFIX);
        if is_png_suffix {
            return MimeType::IMAGE_PNG;
        }

        let is_webp_suffix = request_uri.ends_with(MimeType::WEBP_SUFFIX);
        if is_webp_suffix {
            return MimeType::IMAGE_WEBP;
        }

        let is_bmp_suffix = request_uri.ends_with(MimeType::BMP_SUFFIX);
        if is_bmp_suffix {
            return MimeType::IMAGE_BMP;
        }

        let mut is_ico_suffix = false;
        let boxed_extension = MimeType::get_extension_from_filename(request_uri);
        if !boxed_extension.is_none() {
            let ico_suffixes = vec![MimeType::ICO_SUFFIX, MimeType::CUR_SUFFIX];
            let extension = boxed_extension.unwrap();
            let suffix = [".", extension].join("");
            is_ico_suffix = ico_suffixes.contains(&suffix.as_str())
        }

        if is_ico_suffix {
            return MimeType::IMAGE_ICO;
        }

        let mut is_tiff_suffix = false;
        let boxed_extension = MimeType::get_extension_from_filename(request_uri);
        if !boxed_extension.is_none() {
            let tiff_suffixes = vec![MimeType::TIF_SUFFIX, MimeType::TIFF_SUFFIX];
            let extension = boxed_extension.unwrap();
            let suffix = [".", extension].join("");
            is_tiff_suffix = tiff_suffixes.contains(&suffix.as_str())
        }

        if is_tiff_suffix {
            return MimeType::IMAGE_TIFF;
        }

        let is_aac_suffix = request_uri.ends_with(MimeType::AAC_SUFFIX);
        if is_aac_suffix {
            return MimeType::AUDIO_AAC;
        }

        let is_flac_suffix = request_uri.ends_with(MimeType::FLAC_SUFFIX);
        if is_flac_suffix {
            return MimeType::AUDIO_FLAC;
        }

        let is_wav_suffix = request_uri.ends_with(MimeType::WAV_SUFFIX);
        if is_wav_suffix {
            return MimeType::AUDIO_WAV;
        }

        let is_m4a_suffix = request_uri.ends_with(MimeType::M4A_SUFFIX);
        if is_m4a_suffix {
            return MimeType::AUDIO_MP4;
        }

        let is_oga_suffix = request_uri.ends_with(MimeType::OGA_SUFFIX);
        if is_oga_suffix {
            return MimeType::AUDIO_OGG;
        }

        let is_3gp_suffix = request_uri.ends_with(MimeType::N3GP_SUFFIX);
        if is_3gp_suffix {
            return MimeType::VIDEO_3GP;
        }

        let mut is_mpeg_suffix = false;
        let boxed_extension = MimeType::get_extension_from_filename(request_uri);
        if !boxed_extension.is_none() {
            let mpeg_suffixes = vec![MimeType::MPG_SUFFIX, MimeType::MPEG_SUFFIX];
            let extension = boxed_extension.unwrap();
            let suffix = [".", extension].join("");
            is_mpeg_suffix = mpeg_suffixes.contains(&suffix.as_str())
        }

        if is_mpeg_suffix {
            return MimeType::VIDEO_MPEG;
        }

        let mut is_video_mp4_suffix = false;
        let boxed_extension = MimeType::get_extension_from_filename(request_uri);
        if !boxed_extension.is_none() {
            let mp4_suffixes = vec![MimeType::MP4_SUFFIX, MimeType::M4V_SUFFIX, MimeType::M4P_SUFFIX];
            let extension = boxed_extension.unwrap();
            let suffix = [".", extension].join("");
            is_video_mp4_suffix = mp4_suffixes.contains(&suffix.as_str())
        }

        if is_video_mp4_suffix {
            return MimeType::VIDEO_MP4;
        }

        let mut is_video_ogg_suffix = false;
        let boxed_extension = MimeType::get_extension_from_filename(request_uri);
        if !boxed_extension.is_none() {
            let ogg_suffixes = vec![MimeType::OGG_SUFFIX, MimeType::OGV_SUFFIX];
            let extension = boxed_extension.unwrap();
            let suffix = [".", extension].join("");
            is_video_ogg_suffix = ogg_suffixes.contains(&suffix.as_str())
        }

        if is_video_ogg_suffix {
            return MimeType::VIDEO_OGG;
        }

        let is_mov_suffix = request_uri.ends_with(MimeType::MOV_SUFFIX);
        if is_mov_suffix {
            return MimeType::VIDEO_QUICKTIME;
        }

        let is_webm_suffix = request_uri.ends_with(MimeType::WEBM_SUFFIX);
        if is_webm_suffix {
            return MimeType::VIDEO_WEBM;
        }

        let is_abw_suffix = request_uri.ends_with(MimeType::ABW_SUFFIX);
        if is_abw_suffix {
            return MimeType::APPLICATION_ABIWORD;
        }

        let is_avi_suffix = request_uri.ends_with(MimeType::AVI_SUFFIX);
        if is_avi_suffix {
            return MimeType::VIDEO_X_MSVIDEO;
        }

        let is_azv_suffix = request_uri.ends_with(MimeType::AZV_SUFFIX);
        if is_azv_suffix {
            return MimeType::APPLICATION_VND_AMAZON_EBOOK;
        }

        let is_bin_suffix = request_uri.ends_with(MimeType::BIN_SUFFIX);
        if is_bin_suffix {
            return MimeType::APPLICATION_OCTET_STREAM;
        }

        let is_bz_suffix = request_uri.ends_with(MimeType::BZ_SUFFIX);
        if is_bz_suffix {
            return MimeType::APPLICATION_X_BZIP;
        }

        let is_bz2_suffix = request_uri.ends_with(MimeType::BZ2_SUFFIX);
        if is_bz2_suffix {
            return MimeType::APPLICATION_X_BZIP2;
        }

        let is_cda_suffix = request_uri.ends_with(MimeType::CDA_SUFFIX);
        if is_cda_suffix {
            return MimeType::APPLICATION_X_CDF;
        }

        let is_csh_suffix = request_uri.ends_with(MimeType::CSH_SUFFIX);
        if is_csh_suffix {
            return MimeType::APPLICATION_X_CSH;
        }

        let is_csv_suffix = request_uri.ends_with(MimeType::CSV_SUFFIX);
        if is_csv_suffix {
            return MimeType::TEXT_CSV;
        }

        let is_doc_suffix = request_uri.ends_with(MimeType::DOC_SUFFIX);
        if is_doc_suffix {
            return MimeType::APPLICATION_MSWORD;
        }

        let is_docx_suffix = request_uri.ends_with(MimeType::DOCX_SUFFIX);
        if is_docx_suffix {
            return MimeType::APPLICATION_VND_OPENXMLFORMATS_OFFICEDOCUMENTS_WORDPROCESSINGIMPL_DOCUMENT;
        }

        let is_eot_suffix = request_uri.ends_with(MimeType::EOT_SUFFIX);
        if is_eot_suffix {
            return MimeType::APPLICATION_VND_MS_FONTOBJECT;
        }

        let is_epub_suffix = request_uri.ends_with(MimeType::EPUB_SUFFIX);
        if is_epub_suffix {
            return MimeType::APPLICATION_EPUB_ZIP;
        }

        let is_gz_suffix = request_uri.ends_with(MimeType::GZ_SUFFIX);
        if is_gz_suffix {
            return MimeType::APPLICATION_GZIP;
        }

        let is_ics_suffix = request_uri.ends_with(MimeType::ICS_SUFFIX);
        if is_ics_suffix {
            return MimeType::TEXT_CALENDAR;
        }

        let is_jar_suffix = request_uri.ends_with(MimeType::JAR_SUFFIX);
        if is_jar_suffix {
            return MimeType::APPLICATION_JAVA_ARCHIVE;
        }

        let is_json_suffix = request_uri.ends_with(MimeType::JSON_SUFFIX);
        if is_json_suffix {
            return MimeType::APPLICATION_JSON;
        }

        let is_jsonld_suffix = request_uri.ends_with(MimeType::JSONLD_SUFFIX);
        if is_jsonld_suffix {
            return MimeType::APPLICATION_JSONLD;
        }


        let mut is_midi_suffix = false;
        let boxed_extension = MimeType::get_extension_from_filename(request_uri);
        if !boxed_extension.is_none() {
            let midi_suffixes = vec![MimeType::MIDI_SUFFIX, MimeType::MID_SUFFIX];
            let extension = boxed_extension.unwrap();
            let suffix = [".", extension].join("");
            is_midi_suffix = midi_suffixes.contains(&suffix.as_str())
        }

        if is_midi_suffix {
            return MimeType::AUDIO_MIDI;
        }

        let is_mp3_suffix = request_uri.ends_with(MimeType::MP3_SUFFIX);
        if is_mp3_suffix {
            return MimeType::AUDIO_MPEG;
        }

        let is_mpkg_suffix = request_uri.ends_with(MimeType::MPKG_SUFFIX);
        if is_mpkg_suffix {
            return MimeType::APPLICATION_VND_APPLE_INSTALLER_XML;
        }

        let is_odp_suffix = request_uri.ends_with(MimeType::ODP_SUFFIX);
        if is_odp_suffix {
            return MimeType::APPLICATION_VND_OASIS_OPENDOCUMENT_PRESENTATION;
        }

        let is_ods_suffix = request_uri.ends_with(MimeType::ODS_SUFFIX);
        if is_ods_suffix {
            return MimeType::APPLICATION_VND_OASIS_OPENDOCUMENT_SPREADSHEET;
        }

        let is_odt_suffix = request_uri.ends_with(MimeType::ODT_SUFFIX);
        if is_odt_suffix {
            return MimeType::APPLICATION_VND_OASIS_OPENDOCUMENT_TEXT;
        }

        let is_ogx_suffix = request_uri.ends_with(MimeType::OGX_SUFFIX);
        if is_ogx_suffix {
            return MimeType::APPLICATION_OGG;
        }

        let is_opus_suffix = request_uri.ends_with(MimeType::OPUS_SUFFIX);
        if is_opus_suffix {
            return MimeType::AUDIO_OPUS;
        }

        let is_otf_suffix = request_uri.ends_with(MimeType::OTF_SUFFIX);
        if is_otf_suffix {
            return MimeType::FONT_OTF;
        }

        let is_pdf_suffix = request_uri.ends_with(MimeType::PDF_SUFFIX);
        if is_pdf_suffix {
            return MimeType::APPLICATION_PDF;
        }

        let is_php_suffix = request_uri.ends_with(MimeType::PHP_SUFFIX);
        if is_php_suffix {
            return MimeType::APPLICATION_X_HTTPD_PHP;
        }

        let is_ppt_suffix = request_uri.ends_with(MimeType::PPT_SUFFIX);
        if is_ppt_suffix {
            return MimeType::APPLICATION_VND_MS_POWERPOINT;
        }

        let is_pptx_suffix = request_uri.ends_with(MimeType::PPTX_SUFFIX);
        if is_pptx_suffix {
            return MimeType::APPLICATION_VND_OPENXMLFORMATS_OFFICEDOCUMENT_PRESENTATIONML_PRESENTATION;
        }

        let is_rar_suffix = request_uri.ends_with(MimeType::RAR_SUFFIX);
        if is_rar_suffix {
            return MimeType::APPLICATION_VND_RAR;
        }

        let is_rtf_suffix = request_uri.ends_with(MimeType::RTF_SUFFIX);
        if is_rtf_suffix {
            return MimeType::APPLICATION_RTF;
        }

        let is_sh_suffix = request_uri.ends_with(MimeType::SH_SUFFIX);
        if is_sh_suffix {
            return MimeType::APPLICATION_X_SH;
        }

        let is_swf_suffix = request_uri.ends_with(MimeType::SWF_SUFFIX);
        if is_swf_suffix {
            return MimeType::APPLICATION_X_SHOCKWAVE_FLASH;
        }

        let is_tar_suffix = request_uri.ends_with(MimeType::TAR_SUFFIX);
        if is_tar_suffix {
            return MimeType::APPLICATION_X_TAR;
        }

        let is_ts_suffix = request_uri.ends_with(MimeType::TS_SUFFIX);
        if is_ts_suffix {
            return MimeType::VIDEO_MP2T;
        }

        let is_ttf_suffix = request_uri.ends_with(MimeType::TTF_SUFFIX);
        if is_ttf_suffix {
            return MimeType::FONT_TTF;
        }

        let is_vsd_suffix = request_uri.ends_with(MimeType::VSD_SUFFIX);
        if is_vsd_suffix {
            return MimeType::APPLICATION_VND_VISIO;
        }

        let is_weba_suffix = request_uri.ends_with(MimeType::WEBA_SUFFIX);
        if is_weba_suffix {
            return MimeType::AUDIO_WEBM;
        }

        let is_woff_suffix = request_uri.ends_with(MimeType::WOFF_SUFFIX);
        if is_woff_suffix {
            return MimeType::FONT_WOFF;
        }

        let is_woff2_suffix = request_uri.ends_with(MimeType::WOFF2_SUFFIX);
        if is_woff2_suffix {
            return MimeType::FONT_WOFF2;
        }

        let is_xhtml_suffix = request_uri.ends_with(MimeType::XHTML_SUFFIX);
        if is_xhtml_suffix {
            return MimeType::APPLICATION_XHTML_XML;
        }

        let is_xls_suffix = request_uri.ends_with(MimeType::XLS_SUFFIX);
        if is_xls_suffix {
            return MimeType::APPLICATION_VND_MS_EXCEL;
        }

        let is_xlsx_suffix = request_uri.ends_with(MimeType::XLSX_SUFFIX);
        if is_xlsx_suffix {
            return MimeType::APPLICATION_VND_OPENXMLFORMATS_OFFICEDOCUMENT_SPREADSHEETML_SHEET;
        }

        let is_xml_suffix = request_uri.ends_with(MimeType::XML_SUFFIX);
        if is_xml_suffix {
            return MimeType::APPLICATION_XML;
        }

        let is_xul_suffix = request_uri.ends_with(MimeType::XUL_SUFFIX);
        if is_xul_suffix {
            return MimeType::APPLICATION_VND_MOZILLA_XUL_XML;
        }

        let is_zip_suffix = request_uri.ends_with(MimeType::ZIP_SUFFIX);
        if is_zip_suffix {
            return MimeType::APPLICATION_ZIP;
        }

        let is_zip_suffix = request_uri.ends_with(MimeType::N7Z_SUFFIX);
        if is_zip_suffix {
            return MimeType::APPLICATION_X_7Z_COMPRESSED;
        }

        let is_zip_suffix = request_uri.ends_with(MimeType::N3G2_SUFFIX);
        if is_zip_suffix {
            return MimeType::VIDEO_3GPP2;
        }

        let is_crt_suffix = request_uri.ends_with(MimeType::CRT_SUFFIX);
        if is_crt_suffix {
            return MimeType::APPLICATION_X_X509_CA_CERT;
        }

        let is_wasm_suffix = request_uri.ends_with(MimeType::WASM_SUFFIX);
        if is_wasm_suffix {
            return MimeType::APPLICATION_WASM;
        }

        return MimeType::APPLICATION_OCTET_STREAM;
    }

    pub fn get_extension_from_filename(filename: &str) -> Option<&str> {
        Path::new(filename).extension().and_then(OsStr::to_str)
    }

}


