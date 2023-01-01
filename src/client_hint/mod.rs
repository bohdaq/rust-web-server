#[cfg(test)]
mod tests;

use crate::header::Header;

#[derive(PartialEq, Eq, Clone, Debug)]
pub struct ClientHint;

impl ClientHint {
    pub const ACCEPT_CLIENT_HINTS: &'static str = "Accept-CH";
    pub const CRITICAL_CLIENT_HINTS: &'static str = "Critical-CH";

    pub const USER_AGENT_CPU_ARCHITECTURE: &'static str = "Sec-CH-UA-Arch";
    pub const USER_AGENT_CPU_BITNESS: &'static str = "Sec-CH-UA-Bitness";
    pub const USER_AGENT_FULL_BRAND_INFORMATION: &'static str = "Sec-CH-UA-Full-Version-List";
    pub const USER_AGENT_DEVICE_MODEL: &'static str = "Sec-CH-UA-Model";
    pub const USER_AGENT_OPERATING_SYSTEM_VERSION: &'static str = "Sec-CH-UA-Platform-Version";
    pub const NETWORK_DOWNLOAD_SPEED: &'static str = "Downlink"; // (Mbps)
    pub const NETWORK_EFFECTIVE_CONNECTION_TYPE: &'static str = "ECT"; // (2g/3g/4g)
    pub const NETWORK_ROUND_TRIP_TIME: &'static str = "RTT"; // (in ms, includes server processing time)
    pub const NETWORK_SAVE_DATA: &'static str = "Save-Data";
    pub const DEVICE_MEMORY: &'static str = "Device-Memory";
    pub const PREFERS_REDUCED_MOTION: &'static str = "Sec-CH-Prefers-Reduced-Motion";
    pub const PREFERS_COLOR_SCHEME: &'static str = "Sec-CH-Prefers-Color-Scheme";

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
            ClientHint::NETWORK_SAVE_DATA,
            ClientHint::DEVICE_MEMORY,
            ClientHint::PREFERS_REDUCED_MOTION,
            ClientHint::PREFERS_COLOR_SCHEME,
        ];
        let hint_header_value = hint_list.join(", ");
        hint_header_value
    }

    pub fn get_accept_client_hints_header() -> Header {
        let hint_header_value = ClientHint::get_client_hint_list();
        let header = Header { name: ClientHint::ACCEPT_CLIENT_HINTS.to_string(), value: hint_header_value.to_string() };
        header
    }

    pub fn get_critical_client_hints_header() -> Header {
        let hint_header_value = ClientHint::get_client_hint_list();
        let header = Header { name: ClientHint::CRITICAL_CLIENT_HINTS.to_string(), value: hint_header_value.to_string() };
        header
    }

    pub fn get_vary_header_value() -> String {
        let hint_list = [
            ClientHint::USER_AGENT_CPU_ARCHITECTURE,
            ClientHint::USER_AGENT_CPU_BITNESS,
            ClientHint::USER_AGENT_FULL_BRAND_INFORMATION,
            ClientHint::USER_AGENT_DEVICE_MODEL,
            ClientHint::USER_AGENT_OPERATING_SYSTEM_VERSION,
            ClientHint::NETWORK_SAVE_DATA,
            ClientHint::DEVICE_MEMORY,
            Header::_UPGRADE_INSECURE_REQUESTS,
            ClientHint::PREFERS_REDUCED_MOTION,
            ClientHint::PREFERS_COLOR_SCHEME,
        ];
        let vary_client_hint = hint_list.join(", ");
        vary_client_hint
    }

}