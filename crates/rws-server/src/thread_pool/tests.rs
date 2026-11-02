use crate::thread_pool::ThreadPool;

#[test]
fn thread_pool_test() {
    fn fn_to_execute_by_threadpool() {
        println!("{}", 2 + 2);
    }

    let thread_count : usize = 4;
    let pool = ThreadPool::new(thread_count);
    pool.execute(move ||  {
        fn_to_execute_by_threadpool();
    });
}
#[test]
fn worker_survives_a_panicking_job() {
    use std::sync::mpsc;
    use std::time::Duration;
    // One worker: if the panic killed it, the second job would never run.
    let pool = ThreadPool::new(1);
    pool.execute(|| panic!("handler bug"));
    let (tx, rx) = mpsc::channel();
    pool.execute(move || tx.send(42).unwrap());
    assert_eq!(Ok(42), rx.recv_timeout(Duration::from_secs(5)));
}
