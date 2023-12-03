mod ast;
mod lexer;
mod parser;
mod ptr;
mod token;

fn main() {
    println!("Hello, world!");
    let result = parser::parse();
    match result {
        Err(e) => println!("File open error: {}", e),
        Ok(()) => {}
    }

    println!("Press Enter to exit...");
    let mut input = String::new();
    match std::io::stdin().read_line(&mut input) {
        Ok(_) => (),
        Err(error) => println!("Error reading input: {}", error),
    }
}
