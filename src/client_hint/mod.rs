pub struct AcceptClientHint {
    pub accept_all: bool,
    pub user_agent_cpu_architecture: bool, // Sec-CH-UA-Arch
    pub user_agent_cpu_bitness: bool, // Sec-CH-UA-Bitness
    pub user_agent_full_brand_information: bool, // Sec-CH-UA-Full-Version-List
    pub user_agent_device_model: bool, // Sec-CH-UA-Model
    pub user_agent_operating_system_version: bool, // Sec-CH-UA-Platform-Version
}