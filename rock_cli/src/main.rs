#![forbid(unsafe_code)]

#[allow(dead_code)]
mod ansi;
mod args;
mod cmd;
mod error_format;

pub fn main() {
    match args::parse_args() {
        Ok(cmd::Command::New(data)) => cmd::new::cmd(data),
        Ok(cmd::Command::Check) => cmd::check::cmd(),
        Ok(cmd::Command::Build(data)) => cmd::build::cmd(data),
        Ok(cmd::Command::Run(data)) => cmd::run::cmd(data),
        Ok(cmd::Command::Help) => cmd::help::cmd(),
        Ok(cmd::Command::Version) => cmd::version::cmd(),
        Err(errors) => error_format::print_errors(None, &errors),
    };
}
