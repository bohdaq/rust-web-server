use crate::entry_point::{bootstrap, get_ip_port_thread_count};

#[test]
fn base(){
    bootstrap();
    get_ip_port_thread_count();
}