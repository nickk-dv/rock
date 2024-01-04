use std::sync::{Arc, Mutex};
use std::thread;

pub fn parallel_task<Task, TaskResult, TaskResource>(
    tasks: Vec<Task>,
    task_proc: fn(Task, &mut TaskResource) -> TaskResult,
    task_resource: fn() -> TaskResource,
) -> (Vec<TaskResult>, Vec<TaskResource>)
where
    Task: Send + 'static,
    TaskResult: Send + Default + 'static,
    TaskResource: Send + Copy + 'static,
{
    let task_count = tasks.len();
    let workers = get_thread_count().clamp(0, task_count);
    let tasks_mutex = Arc::new(Mutex::new(tasks));
    let mut handles = Vec::new();

    for _ in 0..workers {
        let arc_tasks = Arc::clone(&tasks_mutex);
        handles.push(thread::spawn(move || {
            let mut results = Vec::new();
            let mut resource = task_resource();
            loop {
                let (task, id) = {
                    let mut lock = arc_tasks.lock().unwrap();
                    match lock.pop() {
                        Some(next_task) => (next_task, lock.len()),
                        None => break,
                    }
                };
                results.push((task_proc(task, &mut resource), id));
            }
            (results, resource)
        }));
    }

    let mut results = Vec::new();
    results.resize_with(task_count, || TaskResult::default());
    let mut resources = Vec::new();

    for handle in handles {
        match handle.join() {
            Ok((result, res)) => {
                resources.push(res);
                for (r, id) in result {
                    results[id] = r;
                }
            }
            Err(err) => {
                println!("failed to join: {:?}", err);
            }
        }
    }
    (results, resources)
}

fn get_thread_count() -> usize {
    match thread::available_parallelism() {
        Ok(count) => count.into(),
        Err(err) => {
            println!("failed to query available threads: {}", err); //@use err interal?
            1
        }
    }
}
