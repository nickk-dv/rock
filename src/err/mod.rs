pub mod ansi;
pub mod error;
pub mod error_new;
mod message;
pub mod range_fmt;
pub mod report;

/*
level_name: `message string`
    --> `path`:`line`:`col`
     |
LINE | `source code line`
     | ________^^^^_`context message`
*/

fn count_digits(mut num: u32) -> usize {
    if num == 0 {
        return 1; // Special case for zero
    }

    let mut count = 0;

    while num != 0 {
        num /= 10;
        count += 1;
    }

    count
}

#[test]
fn test_out() {
    let severity = "error";
    let message = "problem in source";
    let path = "path\\to\\file.lang";
    let line = u32::MAX;
    let col = 2;
    let source_line = "proc main (some: T, ..) {";
    let marker_pad_ = "           ";
    let marker_sym_ = "^^^^";
    let context_msg = "some problem here";

    let line_digits = {
        let mut count = 1;
        let mut num = line;
        while num >= 10 {
            num /= 10;
            count += 1;
        }
        count
    };
    let line_pad = " ".repeat(line_digits);

    #[rustfmt::skip]
    let multi_format = format!(r#"
{severity}: {message}
{line_pad}--> {path}:{line}:{col}
{line_pad} |
{line} | {source_line}
{line_pad} | {marker_pad_}{marker_sym_} {context_msg}"#);
    println!("{multi_format}");
    println!("{multi_format}");

    let range_format = format!("{severity}: {message}\n{line_pad}--> {path}:{line}:{col}\n{line_pad} | \n{line} | {source_line}\n{line_pad} | {marker_pad_}{marker_sym_} {context_msg}");
    //println!("{range_format}");
    //println!("{range_format}");
}
