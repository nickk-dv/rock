mod ast;
mod mem;

use std::time::Instant;

fn main() {
    let now = Instant::now();
    let mut parser = ast::Parser::new();
    let result = parser.parse_package();
    let duration_ms = now.elapsed().as_secs_f64() * 1000.0;
    println!("parse time: ms {}", duration_ms);

    match result {
        Ok(_) => println!("Parse success"),
        Err(()) => print!("Parse failed"),
    }
}
