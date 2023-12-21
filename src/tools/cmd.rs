use crate::ast::parser;

pub fn cmd_parse() -> Result<(), ()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    Ok(())
}

fn cmd_check() -> Result<(), ()> {
    let ast = parser::parse()?;
    Ok(())
}

fn cmd_build() -> Result<(), ()> {
    let ast = parser::parse()?;
    Ok(())
}

fn cmd_run() -> Result<(), ()> {
    let ast = parser::parse()?;
    Ok(())
}

fn cmd_new() -> Result<(), ()> {
    Ok(())
}

fn cmd_fmt() -> Result<(), ()> {
    let ast = parser::parse()?;
    Ok(())
}
