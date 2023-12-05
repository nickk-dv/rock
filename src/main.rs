mod ast;
mod lexer;
mod parser;
mod ptr;
mod token;

fn main() {
    println!("Hello, world!");

    let mut parser = parser::Parser::new();
    let result = parser.parse_package();
    match result {
        Ok(_) => println!("Parse success"),
        Err(()) => print!("Parse failed"),
    }

    println!("Press Enter to exit...");
    let mut input = String::new();
    match std::io::stdin().read_line(&mut input) {
        Ok(_) => (),
        Err(error) => println!("Error reading input: {}", error),
    }
}
