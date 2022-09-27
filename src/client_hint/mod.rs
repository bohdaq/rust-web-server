pub struct AskClientHint {
    pub accept_all: bool,
    pub ask_user_agent_cpu_architecture: bool, // Sec-CH-UA-Arch
    pub ask_user_agent_cpu_bitness: bool, // Sec-CH-UA-Bitness
    pub ask_user_agent_full_brand_information: bool, // Sec-CH-UA-Full-Version-List
    pub ask_user_agent_device_model: bool, // Sec-CH-UA-Model
    pub ask_user_agent_operating_system_version: bool, // Sec-CH-UA-Platform-Version
    pub ask_network_download_speed: bool, // Downlink (Mbps)
    pub ask_effective_connection_type: bool, // ECT (2g/3g/4g)
    pub ask_round_trip_time: bool, // RTT (in ms, includes server processing time)
}