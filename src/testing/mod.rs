use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

pub fn get_thread_count() -> usize {
    match thread::available_parallelism() {
        Ok(count) => count.into(),
        Err(err) => {
            println!("failed to query available threads: {}", err); //@use err interal
            1
        }
    }
}

fn doing_some_work(value: i32) -> i32 {
    let mut i = value;

    for _ in 0..1000000 {
        i *= 2;
        i %= 294;
    }

    i
}

pub fn test_threads() {
    println!("test 1");
    test_threads_1();
    println!("test 2");
    test_threads_2();
    println!("test 3");
    test_threads_3();
}

pub fn test_threads_1() {
    let mut handles = Vec::new();
    let thread_count = get_thread_count();

    for _ in 0..thread_count {
        handles.push(thread::spawn(|| doing_some_work(2)));
    }

    let mut results = Vec::new();
    for handle in handles {
        match handle.join() {
            Ok(result) => {
                results.push(result);
            }
            Err(err) => {
                println!("thread join error: {:?}", err);
            }
        }
    }

    for r in results {
        println!("result {}", r);
    }
}

pub fn test_threads_2() {
    let mut handles = Vec::new();
    let thread_count = get_thread_count();

    let tasks = vec![2, 3, 4, 1, 2, 3, 4, 5, 6];
    let val = 2;

    for _ in 0..thread_count {
        handles.push(thread::spawn(move || doing_some_work(val)));
    }

    let mut results = Vec::new();
    for handle in handles {
        match handle.join() {
            Ok(result) => {
                results.push(result);
            }
            Err(err) => {
                println!("thread join error: {:?}", err);
            }
        }
    }

    for r in results {
        println!("result {}", r);
    }
}

fn work_on_task(tasks: Arc<Mutex<Vec<u64>>>) -> Vec<f32> {
    let mut results = Vec::new();

    loop {
        let task = {
            let mut guard = tasks.lock().unwrap();
            guard.pop()
        };

        match task {
            Some(val) => {
                println!(
                    "Thread {:?} got task, working...{}",
                    thread::current().id(),
                    val
                );
                thread::sleep(Duration::from_secs(val));
                results.push(val as f32);
            }
            None => break,
        }
    }

    results
}

pub fn test_threads_3() {
    let mut handles = Vec::new();
    let thread_count = get_thread_count();

    let tasks = vec![
        1, 2, 3, 4, 5, 6, 11, 12, 13, 1, 15, 16, 2, 6, 7, 8, 9, 1, 13, 2, 3, 4, 5, 6, 7, 8, 3, 5,
        7, 8, 9, 1, 2, 5,
    ];
    let arc_mutex = Arc::new(Mutex::new(tasks));

    for _ in 0..thread_count {
        let arc_copy = Arc::clone(&arc_mutex);
        handles.push(thread::spawn(|| work_on_task(arc_copy)));
    }

    let mut i = 0;
    for handle in handles {
        match handle.join() {
            Ok(result) => {
                println!(
                    "Worker {} results: {:?} sum: {}",
                    i,
                    result,
                    result.iter().copied().sum::<f32>(),
                );
                i += 1;
            }
            Err(err) => {
                println!("thread join error: {:?}", err);
            }
        }
    }
}
