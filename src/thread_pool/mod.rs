#[cfg(test)]
mod tests;

use std::{thread};
use std::sync::{Arc, mpsc, Mutex};

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
        let job = Box::new(f);
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

            let boxed_lock = receiver.lock();
            if boxed_lock.is_err() {
                eprintln!("Worker {} -> unable to acquire lock {}", id, boxed_lock.err().unwrap());
            } else {
                let boxed_job = boxed_lock.unwrap().recv();
                match boxed_job {
                    Ok(job) => job(),
                    Err(_) => break,
                }
            }

        });

        if boxed_thread.is_err() {
            eprintln!("Failed while creating a thread id: {} error: {}", id, boxed_thread.as_ref().err().unwrap());
        }

        Worker { _id: id, _thread: boxed_thread.unwrap() }
    }
}