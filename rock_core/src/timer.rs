use std::time::Instant;

pub struct Timer {
    start: Instant,
    time_ms: f64,
}

impl Timer {
    pub fn new() -> Timer {
        Timer {
            start: Instant::now(),
            time_ms: 9999.0,
        }
    }

    pub fn measure(&mut self) {
        let end = Instant::now();
        self.time_ms = end.duration_since(self.start).as_secs_f64() * 1000.0;
    }

    pub fn display(self, msg: &str) {
        eprintln!("{}: {:.3} ms", msg, self.time_ms);
    }

    pub fn stop(self, msg: &str) {
        let end = Instant::now();
        let ms = end.duration_since(self.start).as_secs_f64() * 1000.0;
        eprintln!("{}: {:.3} ms", msg, ms);
    }
}
