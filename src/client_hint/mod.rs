#[cfg(test)]
mod tests;

use std::env;
use crate::entry_point::Config;
use crate::header::Header;

#[derive(PartialEq, Eq, Clone, Debug)]
pub struct ClientHint;

impl ClientHint {
    pub const ACCEPT_CLIENT_HINTS: &'static str = "Accept-CH";

    pub const USER_AGENT_CPU_ARCHITECTURE: &'static str = "Sec-CH-UA-Arch";
    pub const USER_AGENT_CPU_BITNESS: &'static str = "Sec-CH-UA-Bitness";
    pub const USER_AGENT_FULL_BRAND_INFORMATION: &'static str = "Sec-CH-UA-Full-Version-List";
    pub const USER_AGENT_DEVICE_MODEL: &'static str = "Sec-CH-UA-Model";
    pub const USER_AGENT_OPERATING_SYSTEM_VERSION: &'static str = "Sec-CH-UA-Platform-Version";
    pub const NETWORK_DOWNLOAD_SPEED: &'static str = "Downlink"; // (Mbps)
    pub const NETWORK_EFFECTIVE_CONNECTION_TYPE: &'static str = "ECT"; // (2g/3g/4g)
    pub const NETWORK_ROUND_TRIP_TIME: &'static str = "RTT"; // (in ms, includes server processing time)

    pub fn get_client_hint_list() -> String {
        let hint_list = [
            ClientHint::USER_AGENT_CPU_ARCHITECTURE,
            ClientHint::USER_AGENT_CPU_BITNESS,
            ClientHint::USER_AGENT_FULL_BRAND_INFORMATION,
            ClientHint::USER_AGENT_DEVICE_MODEL,
            ClientHint::USER_AGENT_OPERATING_SYSTEM_VERSION,
            ClientHint::NETWORK_DOWNLOAD_SPEED,
            ClientHint::NETWORK_EFFECTIVE_CONNECTION_TYPE,
            ClientHint::NETWORK_ROUND_TRIP_TIME,
        ];
        let hint_header_value = hint_list.join(", ");
        hint_header_value
    }

    pub fn get_accept_client_hints_header() -> Option<Header> {
        let boxed_rws_config_client_hints = env::var(Config::RWS_CONFIG_CLIENT_HINTS);
        if boxed_rws_config_client_hints.is_ok() {
            let boxed_rws_config_client_hints_as_string = boxed_rws_config_client_hints.unwrap();
            let boxed_rws_config_client_hints_as_bool = boxed_rws_config_client_hints_as_string.parse();
            if boxed_rws_config_client_hints_as_bool.is_err() {
                eprintln!("wrong RWS_CONFIG_CLIENT_HINTS value: {}", boxed_rws_config_client_hints_as_string);
                return None;
            }
            let client_hint_config_value : bool = boxed_rws_config_client_hints_as_bool.unwrap();
            if client_hint_config_value {
                let hint_header_value = ClientHint::get_client_hint_list();
                let header = Header { name: ClientHint::ACCEPT_CLIENT_HINTS.to_string(), value: hint_header_value.to_string() };
                return Some(header);
            }

        }
        None
    }

    pub fn get_vary_header_value() -> String {
        ClientHint::get_client_hint_list()
    }

}