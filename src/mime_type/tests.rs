use crate::mime_type::MimeType;

#[test]
fn detect_mime_type_for_mp4_file() {
    let expected_mime_type = MimeType::VIDEO_MP4;
    let request_uri = "/drahobrat_pt2/drahobrat_pt2_ver2.mp4";

    let actual_mime_type = MimeType::detect_mime_type(request_uri);

    assert_eq!(expected_mime_type, actual_mime_type);
}

#[test]
fn detect_mime_type_for_binary_file() {
    let expected_mime_type = MimeType::APPLICATION_OCTET_STREAM;
    let request_uri = "/rust-web-server/0.0.2/x86_64-unknown-linux-gnu/rws";

    let actual_mime_type = MimeType::detect_mime_type(request_uri);

    assert_eq!(expected_mime_type, actual_mime_type);
}

#[test]
fn detect_mime_type_for_text_file() {
    let expected_mime_type = MimeType::TEXT_PLAIN;
    let request_uri = "/dir/test.txt";

    let actual_mime_type = MimeType::detect_mime_type(request_uri);

    assert_eq!(expected_mime_type, actual_mime_type);
}

#[test]
fn detect_mime_type_for_css_file() {
    let expected_mime_type = MimeType::TEXT_CSS;
    let request_uri = "/dir/test.css";

    let actual_mime_type = MimeType::detect_mime_type(request_uri);

    assert_eq!(expected_mime_type, actual_mime_type);
}

#[test]
fn detect_mime_type_for_html_file() {
    let expected_mime_type = MimeType::TEXT_HTML;
    let request_uri = "/dir/test.html";

    let actual_mime_type = MimeType::detect_mime_type(request_uri);

    assert_eq!(expected_mime_type, actual_mime_type);
}

#[test]
fn detect_mime_type_for_htm_file() {
    let expected_mime_type = MimeType::TEXT_HTML;
    let request_uri = "/dir/test.htm";

    let actual_mime_type = MimeType::detect_mime_type(request_uri);

    assert_eq!(expected_mime_type, actual_mime_type);
}

#[test]
fn detect_mime_type_for_js_file() {
    let expected_mime_type = MimeType::TEXT_JAVASCRIPT;
    let request_uri = "/dir/test.js";

    let actual_mime_type = MimeType::detect_mime_type(request_uri);

    assert_eq!(expected_mime_type, actual_mime_type);
}

#[test]
fn detect_mime_type_for_mjs_file() {
    let expected_mime_type = MimeType::TEXT_JAVASCRIPT;
    let request_uri = "/dir/test.mjs";

    let actual_mime_type = MimeType::detect_mime_type(request_uri);

    assert_eq!(expected_mime_type, actual_mime_type);
}

#[test]
fn detect_mime_type_for_image_apng_file() {
    let expected_mime_type = MimeType::IMAGE_APNG;
    let request_uri = "/dir/test.apng";

    let actual_mime_type = MimeType::detect_mime_type(request_uri);

    assert_eq!(expected_mime_type, actual_mime_type);
}

#[test]
fn detect_mime_type_for_image_avif_file() {
    let expected_mime_type = MimeType::IMAGE_AVIF;
    let request_uri = "/dir/test.avif";

    let actual_mime_type = MimeType::detect_mime_type(request_uri);

    assert_eq!(expected_mime_type, actual_mime_type);
}

#[test]
fn detect_mime_type_for_image_gif_file() {
    let expected_mime_type = MimeType::IMAGE_GIF;
    let request_uri = "/dir/test.gif";

    let actual_mime_type = MimeType::detect_mime_type(request_uri);

    assert_eq!(expected_mime_type, actual_mime_type);
}

#[test]
fn detect_mime_type_for_image_jpg_file() {
    let expected_mime_type = MimeType::IMAGE_JPEG;
    let request_uri = "/dir/test.jpg";

    let actual_mime_type = MimeType::detect_mime_type(request_uri);

    assert_eq!(expected_mime_type, actual_mime_type);
}

#[test]
fn detect_mime_type_for_image_jpeg_file() {
    let expected_mime_type = MimeType::IMAGE_JPEG;
    let request_uri = "/dir/test.jpeg";

    let actual_mime_type = MimeType::detect_mime_type(request_uri);

    assert_eq!(expected_mime_type, actual_mime_type);
}

#[test]
fn detect_mime_type_for_image_jpe_file() {
    let expected_mime_type = MimeType::IMAGE_JPEG;
    let request_uri = "/dir/test.jpe";

    let actual_mime_type = MimeType::detect_mime_type(request_uri);

    assert_eq!(expected_mime_type, actual_mime_type);
}

#[test]
fn detect_mime_type_for_image_jif_file() {
    let expected_mime_type = MimeType::IMAGE_JPEG;
    let request_uri = "/dir/test.jif";

    let actual_mime_type = MimeType::detect_mime_type(request_uri);

    assert_eq!(expected_mime_type, actual_mime_type);
}

#[test]
fn detect_mime_type_for_image_jfif_file() {
    let expected_mime_type = MimeType::IMAGE_JPEG;
    let request_uri = "/dir/test.jfif";

    let actual_mime_type = MimeType::detect_mime_type(request_uri);

    assert_eq!(expected_mime_type, actual_mime_type);
}

#[test]
fn detect_mime_type_for_image_png_file() {
    let expected_mime_type = MimeType::IMAGE_PNG;
    let request_uri = "/dir/test.png";

    let actual_mime_type = MimeType::detect_mime_type(request_uri);

    assert_eq!(expected_mime_type, actual_mime_type);
}

#[test]
fn detect_mime_type_for_image_svg_file() {
    let expected_mime_type = MimeType::IMAGE_PNG;
    let request_uri = "/dir/test.png";

    let actual_mime_type = MimeType::detect_mime_type(request_uri);

    assert_eq!(expected_mime_type, actual_mime_type);
}

#[test]
fn detect_mime_type_for_image_webp_file() {
    let expected_mime_type = MimeType::IMAGE_WEBP;
    let request_uri = "/dir/test.webp";

    let actual_mime_type = MimeType::detect_mime_type(request_uri);

    assert_eq!(expected_mime_type, actual_mime_type);
}

#[test]
fn detect_mime_type_for_image_bmp_file() {
    let expected_mime_type = MimeType::IMAGE_BMP;
    let request_uri = "/dir/test.bmp";

    let actual_mime_type = MimeType::detect_mime_type(request_uri);

    assert_eq!(expected_mime_type, actual_mime_type);
}

#[test]
fn detect_mime_type_for_image_ico_file() {
    let expected_mime_type = MimeType::IMAGE_ICO;
    let request_uri = "/dir/test.ico";

    let actual_mime_type = MimeType::detect_mime_type(request_uri);

    assert_eq!(expected_mime_type, actual_mime_type);
}

#[test]
fn detect_mime_type_for_image_cur_file() {
    let expected_mime_type = MimeType::IMAGE_ICO;
    let request_uri = "/dir/test.cur";

    let actual_mime_type = MimeType::detect_mime_type(request_uri);

    assert_eq!(expected_mime_type, actual_mime_type);
}

#[test]
fn detect_mime_type_for_image_tif_file() {
    let expected_mime_type = MimeType::IMAGE_TIFF;
    let request_uri = "/dir/test.tif";

    let actual_mime_type = MimeType::detect_mime_type(request_uri);

    assert_eq!(expected_mime_type, actual_mime_type);
}

#[test]
fn detect_mime_type_for_image_tiff_file() {
    let expected_mime_type = MimeType::IMAGE_TIFF;
    let request_uri = "/dir/test.tiff";

    let actual_mime_type = MimeType::detect_mime_type(request_uri);

    assert_eq!(expected_mime_type, actual_mime_type);
}

#[test]
fn detect_mime_type_for_audio_aac_file() {
    let expected_mime_type = MimeType::AUDIO_AAC;
    let request_uri = "/dir/test.aac";

    let actual_mime_type = MimeType::detect_mime_type(request_uri);

    assert_eq!(expected_mime_type, actual_mime_type);
}

#[test]
fn detect_mime_type_for_audio_flac_file() {
    let expected_mime_type = MimeType::AUDIO_FLAC;
    let request_uri = "/dir/test.flac";

    let actual_mime_type = MimeType::detect_mime_type(request_uri);

    assert_eq!(expected_mime_type, actual_mime_type);
}

#[test]
fn detect_mime_type_for_audio_wav_file() {
    let expected_mime_type = MimeType::AUDIO_WAV;
    let request_uri = "/dir/test.wav";

    let actual_mime_type = MimeType::detect_mime_type(request_uri);

    assert_eq!(expected_mime_type, actual_mime_type);
}

#[test]
fn detect_mime_type_for_video_3gp_file() {
    let expected_mime_type = MimeType::VIDEO_3GP;
    let request_uri = "/dir/test.3gp";

    let actual_mime_type = MimeType::detect_mime_type(request_uri);

    assert_eq!(expected_mime_type, actual_mime_type);
}

#[test]
fn detect_mime_type_for_video_mpg_file() {
    let expected_mime_type = MimeType::VIDEO_MPEG;
    let request_uri = "/dir/test.mpg";

    let actual_mime_type = MimeType::detect_mime_type(request_uri);

    assert_eq!(expected_mime_type, actual_mime_type);
}

#[test]
fn detect_mime_type_for_video_mpeg_file() {
    let expected_mime_type = MimeType::VIDEO_MPEG;
    let request_uri = "/dir/test.mpeg";

    let actual_mime_type = MimeType::detect_mime_type(request_uri);

    assert_eq!(expected_mime_type, actual_mime_type);
}

#[test]
fn detect_mime_type_for_audio_oga_file() {
    let expected_mime_type = MimeType::AUDIO_OGG;
    let request_uri = "/dir/test.oga";

    let actual_mime_type = MimeType::detect_mime_type(request_uri);

    assert_eq!(expected_mime_type, actual_mime_type);
}

#[test]
fn detect_mime_type_for_video_mp4_file() {
    let expected_mime_type = MimeType::VIDEO_MP4;
    let request_uri = "/dir/test.mp4";

    let actual_mime_type = MimeType::detect_mime_type(request_uri);

    assert_eq!(expected_mime_type, actual_mime_type);
}

#[test]
fn detect_mime_type_for_video_m4v_file() {
    let expected_mime_type = MimeType::VIDEO_MP4;
    let request_uri = "/dir/test.m4v";

    let actual_mime_type = MimeType::detect_mime_type(request_uri);

    assert_eq!(expected_mime_type, actual_mime_type);
}

#[test]
fn detect_mime_type_for_video_ogx_file() {
    let expected_mime_type = MimeType::APPLICATION_OGG;
    let request_uri = "/dir/test.ogx";

    let actual_mime_type = MimeType::detect_mime_type(request_uri);

    assert_eq!(expected_mime_type, actual_mime_type);
}

#[test]
fn detect_mime_type_for_video_opus_file() {
    let expected_mime_type = MimeType::AUDIO_OPUS;
    let request_uri = "/dir/test.opus";

    let actual_mime_type = MimeType::detect_mime_type(request_uri);

    assert_eq!(expected_mime_type, actual_mime_type);
}

#[test]
fn detect_mime_type_for_video_otf_file() {
    let expected_mime_type = MimeType::FONT_OTF;
    let request_uri = "/dir/test.otf";

    let actual_mime_type = MimeType::detect_mime_type(request_uri);

    assert_eq!(expected_mime_type, actual_mime_type);
}

#[test]
fn detect_mime_type_for_video_pdf_file() {
    let expected_mime_type = MimeType::APPLICATION_PDF;
    let request_uri = "/dir/test.pdf";

    let actual_mime_type = MimeType::detect_mime_type(request_uri);

    assert_eq!(expected_mime_type, actual_mime_type);
}

#[test]
fn detect_mime_type_for_video_ogg_file() {
    let expected_mime_type = MimeType::VIDEO_OGG;
    let request_uri = "/dir/test.ogg";

    let actual_mime_type = MimeType::detect_mime_type(request_uri);

    assert_eq!(expected_mime_type, actual_mime_type);
}

#[test]
fn detect_mime_type_for_video_ogv_file() {
    let expected_mime_type = MimeType::VIDEO_OGG;
    let request_uri = "/dir/test.ogv";

    let actual_mime_type = MimeType::detect_mime_type(request_uri);

    assert_eq!(expected_mime_type, actual_mime_type);
}

#[test]
fn detect_mime_type_for_video_m4p_file() {
    let expected_mime_type = MimeType::VIDEO_MP4;
    let request_uri = "/dir/test.m4p";

    let actual_mime_type = MimeType::detect_mime_type(request_uri);

    assert_eq!(expected_mime_type, actual_mime_type);
}

#[test]
fn detect_mime_type_for_video_quicktime_file() {
    let expected_mime_type = MimeType::VIDEO_QUICKTIME;
    let request_uri = "/dir/test.mov";

    let actual_mime_type = MimeType::detect_mime_type(request_uri);

    assert_eq!(expected_mime_type, actual_mime_type);
}

#[test]
fn detect_mime_type_for_video_webm_file() {
    let expected_mime_type = MimeType::VIDEO_WEBM;
    let request_uri = "/dir/test.webm";

    let actual_mime_type = MimeType::detect_mime_type(request_uri);

    assert_eq!(expected_mime_type, actual_mime_type);
}

#[test]
fn detect_mime_type_for_abiword_file() {
    let expected_mime_type = MimeType::APPLICATION_ABIWORD;
    let request_uri = "/dir/test.abw";

    let actual_mime_type = MimeType::detect_mime_type(request_uri);

    assert_eq!(expected_mime_type, actual_mime_type);
}

#[test]
fn detect_mime_type_for_avi_file() {
    let expected_mime_type = MimeType::VIDEO_X_MSVIDEO;
    let request_uri = "/dir/test.avi";

    let actual_mime_type = MimeType::detect_mime_type(request_uri);

    assert_eq!(expected_mime_type, actual_mime_type);
}

#[test]
fn detect_mime_type_for_azv_file() {
    let expected_mime_type = MimeType::APPLICATION_VND_AMAZON_EBOOK;
    let request_uri = "/dir/test.azw";

    let actual_mime_type = MimeType::detect_mime_type(request_uri);

    assert_eq!(expected_mime_type, actual_mime_type);
}

#[test]
fn detect_mime_type_for_bin_file() {
    let expected_mime_type = MimeType::APPLICATION_OCTET_STREAM;
    let request_uri = "/dir/test.bin";

    let actual_mime_type = MimeType::detect_mime_type(request_uri);

    assert_eq!(expected_mime_type, actual_mime_type);
}

#[test]
fn detect_mime_type_for_bz_file() {
    let expected_mime_type = MimeType::APPLICATION_X_BZIP;
    let request_uri = "/dir/test.bz";

    let actual_mime_type = MimeType::detect_mime_type(request_uri);

    assert_eq!(expected_mime_type, actual_mime_type);
}

#[test]
fn detect_mime_type_for_bz2_file() {
    let expected_mime_type = MimeType::APPLICATION_X_BZIP2;
    let request_uri = "/dir/test.bz2";

    let actual_mime_type = MimeType::detect_mime_type(request_uri);

    assert_eq!(expected_mime_type, actual_mime_type);
}

#[test]
fn detect_mime_type_for_cda_file() {
    let expected_mime_type = MimeType::APPLICATION_X_CDF;
    let request_uri = "/dir/test.cda";

    let actual_mime_type = MimeType::detect_mime_type(request_uri);

    assert_eq!(expected_mime_type, actual_mime_type);
}

#[test]
fn detect_mime_type_for_csh_file() {
    let expected_mime_type = MimeType::APPLICATION_X_CSH;
    let request_uri = "/dir/test.csh";

    let actual_mime_type = MimeType::detect_mime_type(request_uri);

    assert_eq!(expected_mime_type, actual_mime_type);
}

#[test]
fn detect_mime_type_for_csv_file() {
    let expected_mime_type = MimeType::TEXT_CSV;
    let request_uri = "/dir/test.csv";

    let actual_mime_type = MimeType::detect_mime_type(request_uri);

    assert_eq!(expected_mime_type, actual_mime_type);
}

#[test]
fn detect_mime_type_for_doc_file() {
    let expected_mime_type = MimeType::APPLICATION_MSWORD;
    let request_uri = "/dir/test.doc";

    let actual_mime_type = MimeType::detect_mime_type(request_uri);

    assert_eq!(expected_mime_type, actual_mime_type);
}

#[test]
fn detect_mime_type_for_docx_file() {
    let expected_mime_type = MimeType::APPLICATION_VND_OPENXMLFORMATS_OFFICEDOCUMENTS_WORDPROCESSINGIMPL_DOCUMENT;
    let request_uri = "/dir/test.docx";

    let actual_mime_type = MimeType::detect_mime_type(request_uri);

    assert_eq!(expected_mime_type, actual_mime_type);
}

#[test]
fn detect_mime_type_for_eot_file() {
    let expected_mime_type = MimeType::APPLICATION_VND_MS_FONTOBJECT;
    let request_uri = "/dir/test.eot";

    let actual_mime_type = MimeType::detect_mime_type(request_uri);

    assert_eq!(expected_mime_type, actual_mime_type);
}

#[test]
fn detect_mime_type_for_gz_file() {
    let expected_mime_type = MimeType::APPLICATION_GZIP;
    let request_uri = "/dir/test.gz";

    let actual_mime_type = MimeType::detect_mime_type(request_uri);

    assert_eq!(expected_mime_type, actual_mime_type);
}

#[test]
fn detect_mime_type_for_ics_file() {
    let expected_mime_type = MimeType::TEXT_CALENDAR;
    let request_uri = "/dir/test.ics";

    let actual_mime_type = MimeType::detect_mime_type(request_uri);

    assert_eq!(expected_mime_type, actual_mime_type);
}

#[test]
fn detect_mime_type_for_jar_file() {
    let expected_mime_type = MimeType::APPLICATION_JAVA_ARCHIVE;
    let request_uri = "/dir/test.jar";

    let actual_mime_type = MimeType::detect_mime_type(request_uri);

    assert_eq!(expected_mime_type, actual_mime_type);
}

#[test]
fn detect_mime_type_for_json_file() {
    let expected_mime_type = MimeType::APPLICATION_JSON;
    let request_uri = "/dir/test.json";

    let actual_mime_type = MimeType::detect_mime_type(request_uri);

    assert_eq!(expected_mime_type, actual_mime_type);
}

#[test]
fn detect_mime_type_for_jsonld_file() {
    let expected_mime_type = MimeType::APPLICATION_JSONLD;
    let request_uri = "/dir/test.jsonld";

    let actual_mime_type = MimeType::detect_mime_type(request_uri);

    assert_eq!(expected_mime_type, actual_mime_type);
}

#[test]
fn detect_mime_type_for_midi_file() {
    let expected_mime_type = MimeType::AUDIO_MIDI;
    let request_uri = "/dir/test.midi";

    let actual_mime_type = MimeType::detect_mime_type(request_uri);

    assert_eq!(expected_mime_type, actual_mime_type);
}

#[test]
fn detect_mime_type_for_mid_file() {
    let expected_mime_type = MimeType::AUDIO_MIDI;
    let request_uri = "/dir/test.mid";

    let actual_mime_type = MimeType::detect_mime_type(request_uri);

    assert_eq!(expected_mime_type, actual_mime_type);
}

#[test]
fn detect_mime_type_for_mp3_file() {
    let expected_mime_type = MimeType::AUDIO_MPEG;
    let request_uri = "/dir/test.mp3";

    let actual_mime_type = MimeType::detect_mime_type(request_uri);

    assert_eq!(expected_mime_type, actual_mime_type);
}

#[test]
fn detect_mime_type_for_mpkg_file() {
    let expected_mime_type = MimeType::APPLICATION_VND_APPLE_INSTALLER_XML;
    let request_uri = "/dir/test.mpkg";

    let actual_mime_type = MimeType::detect_mime_type(request_uri);

    assert_eq!(expected_mime_type, actual_mime_type);
}

#[test]
fn detect_mime_type_for_odp_file() {
    let expected_mime_type = MimeType::APPLICATION_VND_OASIS_OPENDOCUMENT_PRESENTATION;
    let request_uri = "/dir/test.odp";

    let actual_mime_type = MimeType::detect_mime_type(request_uri);

    assert_eq!(expected_mime_type, actual_mime_type);
}

#[test]
fn detect_mime_type_for_ods_file() {
    let expected_mime_type = MimeType::APPLICATION_VND_OASIS_OPENDOCUMENT_SPREADSHEET;
    let request_uri = "/dir/test.ods";

    let actual_mime_type = MimeType::detect_mime_type(request_uri);

    assert_eq!(expected_mime_type, actual_mime_type);
}

#[test]
fn detect_mime_type_for_odt_file() {
    let expected_mime_type = MimeType::APPLICATION_VND_OASIS_OPENDOCUMENT_TEXT;
    let request_uri = "/dir/test.odt";

    let actual_mime_type = MimeType::detect_mime_type(request_uri);

    assert_eq!(expected_mime_type, actual_mime_type);
}

#[test]
fn detect_mime_type_for_php_file() {
    let expected_mime_type = MimeType::APPLICATION_X_HTTPD_PHP;
    let request_uri = "/dir/test.php";

    let actual_mime_type = MimeType::detect_mime_type(request_uri);

    assert_eq!(expected_mime_type, actual_mime_type);
}

#[test]
fn detect_mime_type_for_ppt_file() {
    let expected_mime_type = MimeType::APPLICATION_VND_MS_POWERPOINT;
    let request_uri = "/dir/test.ppt";

    let actual_mime_type = MimeType::detect_mime_type(request_uri);

    assert_eq!(expected_mime_type, actual_mime_type);
}

#[test]
fn detect_mime_type_for_pptx_file() {
    let expected_mime_type = MimeType::APPLICATION_VND_OPENXMLFORMATS_OFFICEDOCUMENT_PRESENTATIONML_PRESENTATION;
    let request_uri = "/dir/test.pptx";

    let actual_mime_type = MimeType::detect_mime_type(request_uri);

    assert_eq!(expected_mime_type, actual_mime_type);
}

#[test]
fn detect_mime_type_for_rar_file() {
    let expected_mime_type = MimeType::APPLICATION_VND_RAR;
    let request_uri = "/dir/test.rar";

    let actual_mime_type = MimeType::detect_mime_type(request_uri);

    assert_eq!(expected_mime_type, actual_mime_type);
}

#[test]
fn detect_mime_type_for_rtf_file() {
    let expected_mime_type = MimeType::APPLICATION_RTF;
    let request_uri = "/dir/test.rtf";

    let actual_mime_type = MimeType::detect_mime_type(request_uri);

    assert_eq!(expected_mime_type, actual_mime_type);
}

#[test]
fn detect_mime_type_for_sh_file() {
    let expected_mime_type = MimeType::APPLICATION_X_SH;
    let request_uri = "/dir/test.sh";

    let actual_mime_type = MimeType::detect_mime_type(request_uri);

    assert_eq!(expected_mime_type, actual_mime_type);
}

#[test]
fn detect_mime_type_for_swf_file() {
    let expected_mime_type = MimeType::APPLICATION_X_SHOCKWAVE_FLASH;
    let request_uri = "/dir/test.swf";

    let actual_mime_type = MimeType::detect_mime_type(request_uri);

    assert_eq!(expected_mime_type, actual_mime_type);
}

#[test]
fn detect_mime_type_for_tar_file() {
    let expected_mime_type = MimeType::APPLICATION_X_TAR;
    let request_uri = "/dir/test.tar";

    let actual_mime_type = MimeType::detect_mime_type(request_uri);

    assert_eq!(expected_mime_type, actual_mime_type);
}

#[test]
fn detect_mime_type_for_ts_file() {
    let expected_mime_type = MimeType::VIDEO_MP2T;
    let request_uri = "/dir/test.ts";

    let actual_mime_type = MimeType::detect_mime_type(request_uri);

    assert_eq!(expected_mime_type, actual_mime_type);
}

#[test]
fn detect_mime_type_for_ttf_file() {
    let expected_mime_type = MimeType::FONT_TTF;
    let request_uri = "/dir/test.ttf";

    let actual_mime_type = MimeType::detect_mime_type(request_uri);

    assert_eq!(expected_mime_type, actual_mime_type);
}

#[test]
fn detect_mime_type_for_vsd_file() {
    let expected_mime_type = MimeType::APPLICATION_VND_VISIO;
    let request_uri = "/dir/test.vsd";

    let actual_mime_type = MimeType::detect_mime_type(request_uri);

    assert_eq!(expected_mime_type, actual_mime_type);
}

#[test]
fn detect_mime_type_for_webm_file() {
    let expected_mime_type = MimeType::AUDIO_WEBM;
    let request_uri = "/dir/test.weba";

    let actual_mime_type = MimeType::detect_mime_type(request_uri);

    assert_eq!(expected_mime_type, actual_mime_type);
}

#[test]
fn detect_mime_type_for_woff_file() {
    let expected_mime_type = MimeType::FONT_WOFF;
    let request_uri = "/dir/test.woff";

    let actual_mime_type = MimeType::detect_mime_type(request_uri);

    assert_eq!(expected_mime_type, actual_mime_type);
}

#[test]
fn detect_mime_type_for_woff2_file() {
    let expected_mime_type = MimeType::FONT_WOFF2;
    let request_uri = "/dir/test.woff2";

    let actual_mime_type = MimeType::detect_mime_type(request_uri);

    assert_eq!(expected_mime_type, actual_mime_type);
}

#[test]
fn detect_mime_type_for_xhtml_file() {
    let expected_mime_type = MimeType::APPLICATION_XHTML_XML;
    let request_uri = "/dir/test.xhtml";

    let actual_mime_type = MimeType::detect_mime_type(request_uri);

    assert_eq!(expected_mime_type, actual_mime_type);
}

#[test]
fn detect_mime_type_for_xls_file() {
    let expected_mime_type = MimeType::APPLICATION_VND_MS_EXCEL;
    let request_uri = "/dir/test.xls";

    let actual_mime_type = MimeType::detect_mime_type(request_uri);

    assert_eq!(expected_mime_type, actual_mime_type);
}

#[test]
fn detect_mime_type_for_xlsx_file() {
    let expected_mime_type = MimeType::APPLICATION_VND_OPENXMLFORMATS_OFFICEDOCUMENT_SPREADSHEETML_SHEET;
    let request_uri = "/dir/test.xlsx";

    let actual_mime_type = MimeType::detect_mime_type(request_uri);

    assert_eq!(expected_mime_type, actual_mime_type);
}

#[test]
fn detect_mime_type_for_xml_file() {
    let expected_mime_type = MimeType::APPLICATION_XML;
    let request_uri = "/dir/test.xml";

    let actual_mime_type = MimeType::detect_mime_type(request_uri);

    assert_eq!(expected_mime_type, actual_mime_type);
}

#[test]
fn detect_mime_type_for_xul_file() {
    let expected_mime_type = MimeType::APPLICATION_VND_MOZILLA_XUL_XML;
    let request_uri = "/dir/test.xul";

    let actual_mime_type = MimeType::detect_mime_type(request_uri);

    assert_eq!(expected_mime_type, actual_mime_type);
}

#[test]
fn detect_mime_type_for_zip_file() {
    let expected_mime_type = MimeType::APPLICATION_ZIP;
    let request_uri = "/dir/test.zip";

    let actual_mime_type = MimeType::detect_mime_type(request_uri);

    assert_eq!(expected_mime_type, actual_mime_type);
}

#[test]
fn detect_mime_type_for_7z_file() {
    let expected_mime_type = MimeType::APPLICATION_X_7Z_COMPRESSED;
    let request_uri = "/dir/test.7z";

    let actual_mime_type = MimeType::detect_mime_type(request_uri);

    assert_eq!(expected_mime_type, actual_mime_type);
}

#[test]
fn detect_mime_type_for_3g2_file() {
    let expected_mime_type = MimeType::VIDEO_3GPP2;
    let request_uri = "/dir/test.3g2";

    let actual_mime_type = MimeType::detect_mime_type(request_uri);

    assert_eq!(expected_mime_type, actual_mime_type);
}
