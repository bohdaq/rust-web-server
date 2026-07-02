use crate::entry_point::{bootstrap, get_ip_port_thread_count};

#[test]
fn base(){
    let _g = crate::test_env::lock();
    bootstrap();
    get_ip_port_thread_count();
}