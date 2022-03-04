use crate::ThreadPool;

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