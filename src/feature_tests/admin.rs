//! `admin::features()` (rws-proxy) reports the facade's build: each feature
//! it names is forwarded to rws-proxy, so its `cfg!` agrees with ours.

#[test]
fn the_admin_feature_list_matches_this_build() {
    let mut expected = Vec::new();
    macro_rules! feature {
        ($($name:literal),*) => { $( if cfg!(feature = $name) { expected.push($name); } )* };
    }
    feature!("http1", "http2", "http3", "acme", "auth", "sso", "sso-saml", "sso-server", "gateway-api", "tower", "openapi", "mimalloc", "brotli", "zstd", "wasm-plugins", "fips", "geo");
    assert_eq!(crate::admin::features(), expected);
}

#[test]
fn the_admin_feature_json_matches_this_build() {
    let json = crate::admin::ops::build_features_json();
    for name in ["http1", "http2", "http3", "acme", "auth", "serde", "macros", "sso", "sso-saml", "sso-server", "gateway-api", "tower", "openapi", "mimalloc", "brotli", "zstd", "wasm-plugins", "geo"] {
        let on = match name {
            "http1" => cfg!(feature = "http1"), "http2" => cfg!(feature = "http2"), "http3" => cfg!(feature = "http3"),
            "acme" => cfg!(feature = "acme"), "auth" => cfg!(feature = "auth"), "serde" => cfg!(feature = "serde"),
            "macros" => cfg!(feature = "macros"), "sso" => cfg!(feature = "sso"), "sso-saml" => cfg!(feature = "sso-saml"),
            "sso-server" => cfg!(feature = "sso-server"), "gateway-api" => cfg!(feature = "gateway-api"),
            "tower" => cfg!(feature = "tower"), "openapi" => cfg!(feature = "openapi"), "mimalloc" => cfg!(feature = "mimalloc"),
            "brotli" => cfg!(feature = "brotli"), "zstd" => cfg!(feature = "zstd"), "geo" => cfg!(feature = "geo"),
            _ => cfg!(feature = "wasm-plugins"),
        };
        assert!(json.contains(&format!("\"{name}\":{on}")), "{name}: {json}");
    }
}
