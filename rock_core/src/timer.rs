use std::time::Instant;

pub struct Timer {
    start: Instant,
}

impl Timer {
    pub fn new() -> Timer {
        Timer {
            start: Instant::now(),
        }
    }

    pub fn stop(self, msg: &str) {
        let end = Instant::now();
        let ms = end.duration_since(self.start).as_secs_f64() * 1000.0;
        eprintln!("{}: {:.3} ms", msg, ms);
    }
}
