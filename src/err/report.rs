use super::ansi::{self, Color};
use super::error::*;
use super::span_fmt;
use crate::ast::CompCtx;
use std::io::{BufWriter, Stderr, Write};

static mut ERR_COUNT: u32 = 0;

pub fn err_status<T>(ok: T) -> Result<T, ()> {
    if unsafe { ERR_COUNT > 0 } {
        Err(())
    } else {
        Ok(ok)
    }
}

pub fn report(handle: &mut BufWriter<Stderr>, error: &Error, ctx: &CompCtx) {
    unsafe { ERR_COUNT += 1 }
    match error {
        Error::Parse(err) => {
            print_error(handle, "parse error");
            let _ = writeln!(handle, "in {}", err.ctx.as_str());
            let _ = write!(handle, "expected: ");
            for (index, token) in err.expected.iter().enumerate() {
                if index < err.expected.len() - 1 {
                    let _ = write!(handle, "`{}`, ", token.as_str());
                } else {
                    let _ = writeln!(handle, "`{}`", token.as_str());
                }
            }
            span_fmt::print(
                handle,
                ctx.file(err.file_id),
                err.got_token.1,
                Some("unexpected token"),
                false,
            );
        }
        Error::Check(err) => {
            print_error(handle, "error");
            let _ = writeln!(handle, "{}", err.message.0);
            if !err.no_source {
                span_fmt::print(handle, ctx.file(err.file_id), err.span, None, false);
                for info in err.info.iter() {
                    match info {
                        CheckErrorInfo::InfoString(info) => {
                            let _ = writeln!(handle, "{}", info);
                        }
                        CheckErrorInfo::Context(context) => {
                            span_fmt::print(
                                handle,
                                ctx.file(context.file_id),
                                context.span,
                                Some(context.marker),
                                true,
                            );
                        }
                    }
                }
            }
            print_help(handle, err.message.1);
        }
        Error::FileIO(err) => {
            print_error(handle, "env error");
            let _ = writeln!(handle, "{}", err.message.0);
            for info in err.info.iter() {
                let _ = writeln!(handle, "{}", info);
            }
            print_help(handle, err.message.1);
        }
        Error::Internal(err) => {
            print_error(handle, "error [internal]");
            let _ = writeln!(handle, "{}", err.message.0);
            for info in err.info.iter() {
                let _ = writeln!(handle, "{}", info);
            }
            print_help(handle, err.message.1);
        }
    }
}

fn print_error(handle: &mut BufWriter<Stderr>, error_name: &'static str) {
    ansi::set_color(handle, Color::BoldRed);
    let _ = write!(handle, "\n{}: ", error_name);
    ansi::reset(handle);
}

fn print_help(handle: &mut BufWriter<Stderr>, help: Option<&'static str>) {
    if let Some(str) = help {
        ansi::set_color(handle, Color::Cyan);
        let _ = write!(handle, "help: ");
        ansi::reset(handle);
        let _ = writeln!(handle, "{}", str);
    }
}
