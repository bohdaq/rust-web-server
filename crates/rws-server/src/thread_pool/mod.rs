#[cfg(test)]
mod tests;

use std::sync::atomic::Ordering;
use std::sync::{Arc, mpsc, Mutex};
use std::thread;

use crate::metrics;

pub struct ThreadPool {
    _workers: Vec<Worker>,
    sender: mpsc::Sender<Job>,
}

type Job = Box<dyn FnOnce() + Send + 'static>;

impl ThreadPool {
    pub fn new(size: usize) -> ThreadPool {
        assert!(size > 0);

        let (sender, receiver) = mpsc::channel();

        let receiver = Arc::new(Mutex::new(receiver));

        let mut workers = Vec::with_capacity(size);
        for id in 0..size {
            workers.push(Worker::new(id, Arc::clone(&receiver)));
        }

        ThreadPool {
            _workers: workers,
            sender,
        }
    }

    pub fn execute<F>(&self, f: F)
        where
            F: FnOnce() + Send  + 'static,
    {
        metrics::THREAD_POOL_QUEUED.fetch_add(1, Ordering::Relaxed);
        let job = Box::new(move || {
            metrics::THREAD_POOL_QUEUED.fetch_sub(1, Ordering::Relaxed);
            f();
        });
        let boxed_send = self.sender.send(job);
        if boxed_send.is_err() {
            eprintln!("unable to send job: {}", boxed_send.err().unwrap());
        } else {
            boxed_send.unwrap()
        }

    }

    /// Drain the pool: stop accepting new jobs and wait for all in-flight
    /// workers to finish. Consumes `self`.
    pub fn join(mut self) {
        drop(self.sender);
        for worker in self._workers.drain(..) {
            if let Err(e) = worker._thread.join() {
                eprintln!("worker thread panicked: {:?}", e);
            }
        }
    }
}

struct Worker {
    _id: usize,
    _thread: thread::JoinHandle<()>,
}

impl Worker {
    fn new(id: usize, receiver: Arc<Mutex<mpsc::Receiver<Job>>>) -> Worker {
        let builder = thread::Builder::new().name(format!("{}", id));

        let boxed_thread = builder.spawn(move || loop {

            // A poisoned lock still guards a usable receiver — recover it
            // rather than spinning on the error forever.
            let boxed_job = receiver.lock().unwrap_or_else(|e| e.into_inner()).recv();
            match boxed_job {
                // A panicking job must not take the worker with it: otherwise
                // each panic permanently shrinks the pool, and a client that
                // can trigger one can stop the server with `size` requests.
                Ok(job) => {
                    if let Err(panic) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(job)) {
                        let message = panic
                            .downcast_ref::<&str>()
                            .map(|s| s.to_string())
                            .or_else(|| panic.downcast_ref::<String>().cloned())
                            .unwrap_or_else(|| "non-string panic payload".to_string());
                        eprintln!("Worker {}: request handler panicked ({}); worker continues", id, message);
                        metrics::record_error();
                    }
                }
                Err(_) => break,
            }

        });

        if boxed_thread.is_err() {
            eprintln!("Failed while creating a thread id: {} error: {}", id, boxed_thread.as_ref().err().unwrap());
        }

        Worker { _id: id, _thread: boxed_thread.unwrap() }
    }
}