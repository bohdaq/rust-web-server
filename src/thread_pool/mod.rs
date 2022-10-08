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
        self.sender.send(job).unwrap();

    }
}

struct Worker {
    _id: usize,
    _thread: thread::JoinHandle<()>,
}

impl Worker {
    fn new(id: usize, receiver: Arc<Mutex<mpsc::Receiver<Job>>>) -> Worker {
        let thread = thread::spawn(move || loop {

            let boxed_lock = receiver.lock();
            if boxed_lock.is_err() {
                eprintln!("Worker {} -> unable to acquire lock {}", id, boxed_lock.err().unwrap());
            } else {
                let boxed_job = boxed_lock.unwrap().recv();
                if boxed_job.is_err() {
                    eprintln!("Worker {} -> unable to get job to execute {}", id, boxed_job.err().unwrap());
                } else {
                    let job = boxed_job.unwrap();

                    println!("Worker {} got a job; executing.", id);

                    job();
                }

            }



        });

        Worker { _id: id, _thread: thread }
    }
}