use crate::client_hint::ClientHint;
use crate::entry_point::{override_environment_variables_from_config};

#[test]
fn consts() {
    assert_eq!(ClientHint::ACCEPT_CLIENT_HINTS, "Accept-CH");
    assert_eq!(ClientHint::USER_AGENT_CPU_ARCHITECTURE, "Sec-CH-UA-Arch");
    assert_eq!(ClientHint::USER_AGENT_CPU_BITNESS, "Sec-CH-UA-Bitness");
    assert_eq!(ClientHint::USER_AGENT_FULL_BRAND_INFORMATION, "Sec-CH-UA-Full-Version-List");
    assert_eq!(ClientHint::USER_AGENT_DEVICE_MODEL, "Sec-CH-UA-Model");
    assert_eq!(ClientHint::USER_AGENT_OPERATING_SYSTEM_VERSION, "Sec-CH-UA-Platform-Version");
    assert_eq!(ClientHint::NETWORK_DOWNLOAD_SPEED, "Downlink");
    assert_eq!(ClientHint::NETWORK_EFFECTIVE_CONNECTION_TYPE, "ECT");
    assert_eq!(ClientHint::NETWORK_ROUND_TRIP_TIME, "RTT");
}

#[test]
fn hint_list() {
    assert_eq!(ClientHint::get_client_hint_list(), "Sec-CH-UA-Arch, Sec-CH-UA-Bitness, Sec-CH-UA-Full-Version-List, Sec-CH-UA-Model, Sec-CH-UA-Platform-Version, Downlink, ECT, RTT");
}

#[test]
fn client_hints_header() {
    override_environment_variables_from_config(Some("/src/test/client_hint/rws.config.toml"));

    let header = ClientHint::get_accept_client_hints_header();
    assert_eq!(header.name, ClientHint::ACCEPT_CLIENT_HINTS);
    assert_eq!(header.value, "Sec-CH-UA-Arch, Sec-CH-UA-Bitness, Sec-CH-UA-Full-Version-List, Sec-CH-UA-Model, Sec-CH-UA-Platform-Version, Downlink, ECT, RTT");
}

#[test]
fn client_hints_false() {
    let header = ClientHint::get_accept_client_hints_header();
    let hint_header_value = ClientHint::get_client_hint_list();
    assert_eq!(header.value, hint_header_value);
    assert_eq!(header.name, ClientHint::ACCEPT_CLIENT_HINTS);
}


#[test]
fn vary() {
    assert_eq!(ClientHint::get_vary_header_value(), "Sec-CH-UA-Arch, Sec-CH-UA-Bitness, Sec-CH-UA-Full-Version-List, Sec-CH-UA-Model, Sec-CH-UA-Platform-Version");
}