use std::ffi::OsStr;
use std::path::Path;

pub struct MimeType {}


impl MimeType {
    pub(crate) const APPLICATION_OCTET_STREAM: &'static str = "application/octet-stream";
    pub(crate) const APPLICATION_ABIWORD: &'static str = "application/x-abiword";
    pub(crate) const APPLICATION_VND_AMAZON_EBOOK: &'static str = "application/vnd.amazon.ebook";
    pub(crate) const APPLICATION_X_BZIP: &'static str = "application/x-bzip";
    pub(crate) const APPLICATION_X_BZIP2: &'static str = "application/x-bzip2";
    pub(crate) const APPLICATION_X_CDF: &'static str = "application/x-cdf";
    pub(crate) const APPLICATION_X_CSH: &'static str = "application/x-csh";
    pub(crate) const APPLICATION_MSWORD: &'static str = "application/msword";
    pub(crate) const APPLICATION_VND_OPENXMLFORMATS_OFFICEDOCUMENTS_WORDPROCESSINGIMPL_DOCUMENT: &'static str = "application/vnd.openxmlformats-officedocument.wordprocessingml.document";
    pub(crate) const APPLICATION_VND_MS_FONTOBJECT: &'static str = "application/vnd.ms-fontobject";
    pub(crate) const APPLICATION_EPUB_ZIP: &'static str = "application/epub+zip";
    pub(crate) const APPLICATION_GZIP: &'static str = "application/gzip";
    pub(crate) const APPLICATION_JAVA_ARCHIVE: &'static str = "application/java-archive";
    pub(crate) const APPLICATION_JSON: &'static str = "application/json";
    pub(crate) const APPLICATION_JSONLD: &'static str = "application/ld+json";


    pub(crate) const TEXT_PLAIN: &'static str = "text/plain";
    pub(crate) const TEXT_CSS: &'static str = "text/css";
    pub(crate) const TEXT_CSV: &'static str = "text/csv";
    pub(crate) const TEXT_HTML: &'static str = "text/html";
    pub(crate) const TEXT_JAVASCRIPT: &'static str = "text/javascript";
    pub(crate) const TEXT_CALENDAR: &'static str = "text/calendar";


    pub(crate) const IMAGE_APNG: &'static str = "image/apng";
    pub(crate) const IMAGE_AVIF: &'static str = "image/avif";
    pub(crate) const IMAGE_GIF: &'static str = "image/gif";
    pub(crate) const IMAGE_JPEG: &'static str = "image/jpeg";
    pub(crate) const IMAGE_PNG: &'static str = "image/png";
    pub(crate) const IMAGE_SVG: &'static str = "image/svg+xml";
    pub(crate) const IMAGE_WEBP: &'static str = "image/webp";
    pub(crate) const IMAGE_BMP: &'static str = "image/bmp";
    pub(crate) const IMAGE_ICO: &'static str = "image/x-icon";
    pub(crate) const IMAGE_TIFF: &'static str = "image/tiff";


    pub(crate) const AUDIO_AAC: &'static str = "audio/aac";
    pub(crate) const AUDIO_FLAC: &'static str = "audio/flac";
    pub(crate) const AUDIO_WAV: &'static str = "audio/wav";
    pub(crate) const AUDIO_MP4: &'static str = "audio/mp4";
    pub(crate) const AUDIO_OGG: &'static str = "audio/oga";
    pub(crate) const AUDIO_MIDI: &'static str = "audio/midi";



    pub(crate) const VIDEO_3GP: &'static str = "video/3gpp";
    pub(crate) const VIDEO_MPEG: &'static str = "video/mpeg";
    pub(crate) const VIDEO_MP4: &'static str = "video/mp4";
    pub(crate) const VIDEO_OGG: &'static str = "video/ogg";
    pub(crate) const VIDEO_QUICKTIME: &'static str = "video/quicktime";
    pub(crate) const VIDEO_WEBM: &'static str = "video/webm";
    pub(crate) const VIDEO_X_MSVIDEO: &'static str = "video/x-msvideo";


    const TXT_SUFFIX: &'static str = ".txt";
    const CSS_SUFFIX: &'static str = ".css";
    const HTML_SUFFIX: &'static str = ".html";
    const HTM_SUFFIX: &'static str = ".htm";
    const JS_SUFFIX: &'static str = ".js";
    const APNG_SUFFIX: &'static str = ".apng";
    const AVIF_SUFFIX: &'static str = ".avif";
    const GIF_SUFFIX: &'static str = ".gif";
    const JPG_SUFFIX: &'static str = ".jpg";
    const JPEG_SUFFIX: &'static str = ".jpeg";
    const JPE_SUFFIX: &'static str = ".jpe";
    const JIF_SUFFIX: &'static str = ".jif";
    const JFIF_SUFFIX: &'static str = ".jfif";
    const PNG_SUFFIX: &'static str = ".png";
    const SVG_SUFFIX: &'static str = ".svg";
    const WEBP_SUFFIX: &'static str = ".webp";
    const BMP_SUFFIX: &'static str = ".bmp";
    const ICO_SUFFIX: &'static str = ".ico";
    const CUR_SUFFIX: &'static str = ".cur";
    const TIF_SUFFIX: &'static str = ".tif";
    const TIFF_SUFFIX: &'static str = ".tiff";
    const AAC_SUFFIX: &'static str = ".aac";
    const FLAC_SUFFIX: &'static str = ".flac";
    const WAV_SUFFIX: &'static str = ".wav";
    const M4A_SUFFIX: &'static str = ".m4a";
    const N3GP_SUFFIX: &'static str = ".3gp";
    const MPG_SUFFIX: &'static str = ".mpg";
    const MPEG_SUFFIX: &'static str = ".mpeg";
    const MP4_SUFFIX: &'static str = ".mp4";
    const M4V_SUFFIX: &'static str = ".m4v";
    const M4P_SUFFIX: &'static str = ".m4p";
    const OGA_SUFFIX: &'static str = ".oga";
    const OGG_SUFFIX: &'static str = ".ogg";
    const OGV_SUFFIX: &'static str = ".ogv";
    const MOV_SUFFIX: &'static str = ".mov";
    const WEBM_SUFFIX: &'static str = ".webm";
    const ABW_SUFFIX: &'static str = ".abw";
    const AVI_SUFFIX: &'static str = ".avi";
    const AZV_SUFFIX: &'static str = ".azw";
    const BIN_SUFFIX: &'static str = ".bin";
    const BZ_SUFFIX: &'static str = ".bz";
    const BZ2_SUFFIX: &'static str = ".bz2";
    const CDA_SUFFIX: &'static str = ".cda";
    const CSH_SUFFIX: &'static str = ".csh";
    const CSV_SUFFIX: &'static str = ".csv";
    const DOC_SUFFIX: &'static str = ".doc";
    const DOCX_SUFFIX: &'static str = ".docx";
    const EOT_SUFFIX: &'static str = ".eot";
    const EPUB_SUFFIX: &'static str = ".epub";
    const GZ_SUFFIX: &'static str = ".gz";
    const ICS_SUFFIX: &'static str = ".ics";
    const JAR_SUFFIX: &'static str = ".jar";
    const JSON_SUFFIX: &'static str = ".json";
    const JSONLD_SUFFIX: &'static str = ".jsonld";
    const MIDI_SUFFIX: &'static str = ".midi";
    const MID_SUFFIX: &'static str = ".mid";



    pub(crate) fn detect_mime_type(request_uri: &str) -> String {

        let is_txt_suffix = request_uri.ends_with(MimeType::TXT_SUFFIX);
        if is_txt_suffix {
            return MimeType::TEXT_PLAIN.to_string();
        }

        let is_css_suffix = request_uri.ends_with(MimeType::CSS_SUFFIX);
        if is_css_suffix {
            return MimeType::TEXT_CSS.to_string();
        }

        let mut is_html_suffix = false;
        let boxed_extension = MimeType::get_extension_from_filename(request_uri);
        if !boxed_extension.is_none() {
            let HTML_SUFFIXES = vec![MimeType::HTML_SUFFIX, MimeType::HTM_SUFFIX];
            let extension = boxed_extension.unwrap();
            let suffix = [".", extension].join("");
            is_html_suffix = HTML_SUFFIXES.contains(&suffix.as_str())
        }

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

        let is_gif_suffix = request_uri.ends_with(MimeType::GIF_SUFFIX);
        if is_gif_suffix {
            return MimeType::IMAGE_GIF.to_string();
        }

        let is_svg_suffix = request_uri.ends_with(MimeType::SVG_SUFFIX);
        if is_svg_suffix {
            return MimeType::IMAGE_SVG.to_string();
        }

        let mut is_jpeg_suffix = false;
        let boxed_extension = MimeType::get_extension_from_filename(request_uri);
        if !boxed_extension.is_none() {
            let JPEG_SUFFIXES = vec![MimeType::JPG_SUFFIX, MimeType::JPEG_SUFFIX, MimeType::JPE_SUFFIX, MimeType::JIF_SUFFIX, MimeType::JFIF_SUFFIX];
            let extension = boxed_extension.unwrap();
            let suffix = [".", extension].join("");
            is_jpeg_suffix = JPEG_SUFFIXES.contains(&suffix.as_str())
        }

        if is_jpeg_suffix {
            return MimeType::IMAGE_JPEG.to_string();
        }

        let is_png_suffix = request_uri.ends_with(MimeType::PNG_SUFFIX);
        if is_png_suffix {
            return MimeType::IMAGE_PNG.to_string();
        }

        let is_webp_suffix = request_uri.ends_with(MimeType::WEBP_SUFFIX);
        if is_webp_suffix {
            return MimeType::IMAGE_WEBP.to_string();
        }

        let is_bmp_suffix = request_uri.ends_with(MimeType::BMP_SUFFIX);
        if is_bmp_suffix {
            return MimeType::IMAGE_BMP.to_string();
        }

        let mut is_ico_suffix = false;
        let boxed_extension = MimeType::get_extension_from_filename(request_uri);
        if !boxed_extension.is_none() {
            let ICO_SUFFIXES = vec![MimeType::ICO_SUFFIX, MimeType::CUR_SUFFIX];
            let extension = boxed_extension.unwrap();
            let suffix = [".", extension].join("");
            is_ico_suffix = ICO_SUFFIXES.contains(&suffix.as_str())
        }

        if is_ico_suffix {
            return MimeType::IMAGE_ICO.to_string();
        }

        let mut is_tiff_suffix = false;
        let boxed_extension = MimeType::get_extension_from_filename(request_uri);
        if !boxed_extension.is_none() {
            let TIFF_SUFFIXES = vec![MimeType::TIF_SUFFIX, MimeType::TIFF_SUFFIX];
            let extension = boxed_extension.unwrap();
            let suffix = [".", extension].join("");
            is_tiff_suffix = TIFF_SUFFIXES.contains(&suffix.as_str())
        }

        if is_tiff_suffix {
            return MimeType::IMAGE_TIFF.to_string();
        }

        let is_aac_suffix = request_uri.ends_with(MimeType::AAC_SUFFIX);
        if is_aac_suffix {
            return MimeType::AUDIO_AAC.to_string();
        }

        let is_flac_suffix = request_uri.ends_with(MimeType::FLAC_SUFFIX);
        if is_flac_suffix {
            return MimeType::AUDIO_FLAC.to_string();
        }

        let is_wav_suffix = request_uri.ends_with(MimeType::WAV_SUFFIX);
        if is_wav_suffix {
            return MimeType::AUDIO_WAV.to_string();
        }

        let is_m4a_suffix = request_uri.ends_with(MimeType::M4A_SUFFIX);
        if is_m4a_suffix {
            return MimeType::AUDIO_MP4.to_string();
        }

        let is_oga_suffix = request_uri.ends_with(MimeType::OGA_SUFFIX);
        if is_oga_suffix {
            return MimeType::AUDIO_OGG.to_string();
        }

        let is_3gp_suffix = request_uri.ends_with(MimeType::N3GP_SUFFIX);
        if is_3gp_suffix {
            return MimeType::VIDEO_3GP.to_string();
        }

        let mut is_mpeg_suffix = false;
        let boxed_extension = MimeType::get_extension_from_filename(request_uri);
        if !boxed_extension.is_none() {
            let MPEG_SUFFIXES = vec![MimeType::MPG_SUFFIX, MimeType::MPEG_SUFFIX];
            let extension = boxed_extension.unwrap();
            let suffix = [".", extension].join("");
            is_mpeg_suffix = MPEG_SUFFIXES.contains(&suffix.as_str())
        }

        if is_mpeg_suffix {
            return MimeType::VIDEO_MPEG.to_string();
        }

        let mut is_video_mp4_suffix = false;
        let boxed_extension = MimeType::get_extension_from_filename(request_uri);
        if !boxed_extension.is_none() {
            let MP4_SUFFIXES = vec![MimeType::MP4_SUFFIX, MimeType::M4V_SUFFIX, MimeType::M4P_SUFFIX];
            let extension = boxed_extension.unwrap();
            let suffix = [".", extension].join("");
            is_video_mp4_suffix = MP4_SUFFIXES.contains(&suffix.as_str())
        }

        if is_video_mp4_suffix {
            return MimeType::VIDEO_MP4.to_string();
        }

        let mut is_video_ogg_suffix = false;
        let boxed_extension = MimeType::get_extension_from_filename(request_uri);
        if !boxed_extension.is_none() {
            let OGG_SUFFIXES = vec![MimeType::OGG_SUFFIX, MimeType::OGV_SUFFIX];
            let extension = boxed_extension.unwrap();
            let suffix = [".", extension].join("");
            is_video_ogg_suffix = OGG_SUFFIXES.contains(&suffix.as_str())
        }

        if is_video_ogg_suffix {
            return MimeType::VIDEO_OGG.to_string();
        }

        let is_mov_suffix = request_uri.ends_with(MimeType::MOV_SUFFIX);
        if is_mov_suffix {
            return MimeType::VIDEO_QUICKTIME.to_string();
        }

        let is_webm_suffix = request_uri.ends_with(MimeType::WEBM_SUFFIX);
        if is_webm_suffix {
            return MimeType::VIDEO_WEBM.to_string();
        }

        let is_abw_suffix = request_uri.ends_with(MimeType::ABW_SUFFIX);
        if is_abw_suffix {
            return MimeType::APPLICATION_ABIWORD.to_string();
        }

        let is_avi_suffix = request_uri.ends_with(MimeType::AVI_SUFFIX);
        if is_avi_suffix {
            return MimeType::VIDEO_X_MSVIDEO.to_string();
        }

        let is_azv_suffix = request_uri.ends_with(MimeType::AZV_SUFFIX);
        if is_azv_suffix {
            return MimeType::APPLICATION_VND_AMAZON_EBOOK.to_string();
        }

        let is_bin_suffix = request_uri.ends_with(MimeType::BIN_SUFFIX);
        if is_bin_suffix {
            return MimeType::APPLICATION_OCTET_STREAM.to_string();
        }

        let is_bz_suffix = request_uri.ends_with(MimeType::BZ_SUFFIX);
        if is_bz_suffix {
            return MimeType::APPLICATION_X_BZIP.to_string();
        }

        let is_bz2_suffix = request_uri.ends_with(MimeType::BZ2_SUFFIX);
        if is_bz2_suffix {
            return MimeType::APPLICATION_X_BZIP2.to_string();
        }

        let is_cda_suffix = request_uri.ends_with(MimeType::CDA_SUFFIX);
        if is_cda_suffix {
            return MimeType::APPLICATION_X_CDF.to_string();
        }

        let is_csh_suffix = request_uri.ends_with(MimeType::CSH_SUFFIX);
        if is_csh_suffix {
            return MimeType::APPLICATION_X_CSH.to_string();
        }

        let is_csv_suffix = request_uri.ends_with(MimeType::CSV_SUFFIX);
        if is_csv_suffix {
            return MimeType::TEXT_CSV.to_string();
        }

        let is_doc_suffix = request_uri.ends_with(MimeType::DOC_SUFFIX);
        if is_doc_suffix {
            return MimeType::APPLICATION_MSWORD.to_string();
        }

        let is_docx_suffix = request_uri.ends_with(MimeType::DOCX_SUFFIX);
        if is_docx_suffix {
            return MimeType::APPLICATION_VND_OPENXMLFORMATS_OFFICEDOCUMENTS_WORDPROCESSINGIMPL_DOCUMENT.to_string();
        }

        let is_eot_suffix = request_uri.ends_with(MimeType::EOT_SUFFIX);
        if is_eot_suffix {
            return MimeType::APPLICATION_VND_MS_FONTOBJECT.to_string();
        }

        let is_epub_suffix = request_uri.ends_with(MimeType::EPUB_SUFFIX);
        if is_epub_suffix {
            return MimeType::APPLICATION_EPUB_ZIP.to_string();
        }

        let is_gz_suffix = request_uri.ends_with(MimeType::GZ_SUFFIX);
        if is_gz_suffix {
            return MimeType::APPLICATION_GZIP.to_string();
        }

        let is_ics_suffix = request_uri.ends_with(MimeType::ICS_SUFFIX);
        if is_ics_suffix {
            return MimeType::TEXT_CALENDAR.to_string();
        }

        let is_jar_suffix = request_uri.ends_with(MimeType::JAR_SUFFIX);
        if is_jar_suffix {
            return MimeType::APPLICATION_JAVA_ARCHIVE.to_string();
        }

        let is_json_suffix = request_uri.ends_with(MimeType::JSON_SUFFIX);
        if is_json_suffix {
            return MimeType::APPLICATION_JSON.to_string();
        }

        let is_jsonld_suffix = request_uri.ends_with(MimeType::JSONLD_SUFFIX);
        if is_jsonld_suffix {
            return MimeType::APPLICATION_JSONLD.to_string();
        }


        let mut is_midi_suffix = false;
        let boxed_extension = MimeType::get_extension_from_filename(request_uri);
        if !boxed_extension.is_none() {
            let MIDI_SUFFIXES = vec![MimeType::MIDI_SUFFIX, MimeType::MID_SUFFIX];
            let extension = boxed_extension.unwrap();
            let suffix = [".", extension].join("");
            is_midi_suffix = MIDI_SUFFIXES.contains(&suffix.as_str())
        }

        if is_midi_suffix {
            return MimeType::AUDIO_MIDI.to_string();
        }

        return MimeType::APPLICATION_OCTET_STREAM.to_string();
    }

    pub(crate) fn get_extension_from_filename(filename: &str) -> Option<&str> {
        Path::new(filename).extension().and_then(OsStr::to_str)
    }

}


