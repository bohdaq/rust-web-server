use crate::entry_point::environment_variables::read_system_environment_variables;

#[test]
fn env_vars() {
    read_system_environment_variables();
}