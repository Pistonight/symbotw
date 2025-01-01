use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};

pub fn num_threads() -> usize {
    num_cpus::get().saturating_sub(1).max(1)
}

pub struct Pool<TIn, TInIter: IntoIterator<Item = TIn>, TOut> {
    jobs: TInIter::IntoIter,
    workers: Vec<Worker<TIn>>,
    recv: Receiver<(usize, TOut)>,
}

impl<
        TIn: Send + 'static,
        TInIter: IntoIterator<Item = TIn> + Send + 'static,
        TOut: Send + 'static,
    > Pool<TIn, TInIter, TOut>
{
    pub fn run(jobs: TInIter, f: fn(TIn) -> TOut) -> Self {
        let (send, recv) = mpsc::channel();
        let num = num_threads();
        let mut workers = Vec::with_capacity(num);
        let mut jobs = jobs.into_iter();
        for id in 0..num {
            let send_out = send.clone();
            if let Some(job) = jobs.next() {
                let worker = Worker::new(id, f, send_out);
                worker.send(job);
                workers.push(worker);
            }
        }
        drop(send);

        Self {
            jobs,
            workers,
            recv,
        }
    }
}

impl<TIn: Send + 'static, TInIter: IntoIterator<Item = TIn>, TOut> Iterator
    for Pool<TIn, TInIter, TOut>
{
    type Item = TOut;

    fn next(&mut self) -> Option<Self::Item> {
        let (i, out) = match self.recv.recv() {
            Ok(x) => x,
            Err(_) => return None,
        };
        if let Some(job) = self.jobs.next() {
            self.workers[i].send(job);
        } else {
            self.workers[i].done();
        }
        Some(out)
    }
}

pub struct Worker<TIn> {
    send: Option<Sender<TIn>>,
    thread: Option<JoinHandle<()>>,
}

impl<TIn: Send + 'static> Worker<TIn> {
    pub fn new<TOut: Send + 'static, TWork: Fn(TIn) -> TOut + Send + 'static>(
        id: usize,
        work: TWork,
        send_out: Sender<(usize, TOut)>,
    ) -> Self {
        let (send, recv) = mpsc::channel();
        let thread = thread::spawn(move || {
            for job in recv {
                let out = work(job);
                let _ = send_out.send((id, out));
            }
        });
        Self {
            send: Some(send),
            thread: Some(thread),
        }
    }

    pub fn send(&self, job: TIn) {
        let _ = self.send.as_ref().unwrap().send(job);
    }

    pub fn done(&mut self) {
        self.send = None;
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}
