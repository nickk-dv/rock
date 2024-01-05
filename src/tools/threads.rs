use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

pub type TaskID = u32;

pub struct ThreadPool<Input, Output, Resource>
where
    Input: Send + 'static,
    Output: Send + 'static,
    Resource: Send + 'static,
{
    task_proc: fn(Input, TaskID, &mut Resource) -> Output,
    resource_proc: fn() -> Resource,
    workers: Vec<JoinHandle<(Vec<(Output, TaskID)>, Resource)>>,
}

impl<Input, Output, Resource> ThreadPool<Input, Output, Resource>
where
    Input: Send + 'static,
    Output: Send + 'static,
    Resource: Send + 'static,
{
    pub fn new(
        task_proc: fn(Input, TaskID, &mut Resource) -> Output,
        resource_proc: fn() -> Resource,
    ) -> Self {
        Self {
            task_proc,
            resource_proc,
            workers: Vec::new(),
        }
    }

    pub fn execute(mut self, tasks: Vec<Input>) -> (Vec<Output>, Vec<Resource>) {
        let task_count = tasks.len();
        let worker_count = Self::worker_count(task_count);

        let task_queue = Arc::new(Mutex::new(tasks));
        for _ in 0..worker_count {
            self.spawn_worker(&task_queue);
        }
        self.join_output(task_count)
    }

    fn worker_count(task_count: usize) -> usize {
        let worker_count = match thread::available_parallelism() {
            Ok(count) => count.into(),
            Err(err) => {
                println!(
                    "thread::available_parallelism() failed, falling back on 1 thread: {}",
                    err
                );
                1
            }
        };
        worker_count.clamp(0, task_count)
    }

    fn spawn_worker(&mut self, task_queue: &Arc<Mutex<Vec<Input>>>) {
        let work_queue = Arc::clone(task_queue);
        let task_proc = self.task_proc;
        let resource_proc = self.resource_proc;

        let worker = thread::spawn(move || {
            let mut results = Vec::new();
            let mut resource = resource_proc();
            loop {
                let (task, task_id) = {
                    let mut tasks = work_queue.lock().unwrap();
                    match tasks.pop() {
                        Some(next_task) => (next_task, tasks.len() as TaskID),
                        None => break,
                    }
                };
                let output = task_proc(task, task_id, &mut resource);
                results.push((output, task_id));
            }
            (results, resource)
        });
        self.workers.push(worker);
    }

    fn join_output(self, task_count: usize) -> (Vec<Output>, Vec<Resource>) {
        let mut results = Vec::new();
        let mut resources = Vec::new();
        results.resize_with(task_count, || None);

        for handle in self.workers {
            match handle.join() {
                Ok((result, res)) => {
                    resources.push(res);
                    for (r, id) in result {
                        results[id as usize] = Some(r);
                    }
                }
                Err(err) => {
                    panic!("Failed to join a worker thread: {:?}", err);
                }
            }
        }

        let output = results.into_iter().map(|output| output.unwrap()).collect();
        (output, resources)
    }
}
