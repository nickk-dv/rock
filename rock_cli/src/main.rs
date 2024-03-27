#![forbid(unsafe_code)]

#[allow(dead_code)]
mod ansi;
mod args;
mod cmd;
mod error_format;

pub fn main() {
    let cmd_result = match args::parse_args() {
        Ok(cmd::Command::New(data)) => cmd::new::cmd(data).map_err(|error| vec![error]),
        Ok(cmd::Command::Check) => cmd::check::cmd(),
        Ok(cmd::Command::Build(data)) => cmd::build::cmd(data),
        Ok(cmd::Command::Run(data)) => cmd::run::cmd(data),
        Ok(cmd::Command::Help) => {
            cmd::help::cmd();
            Ok(())
        }
        Ok(cmd::Command::Version) => {
            cmd::version::cmd();
            Ok(())
        }
        Err(errors) => Err(errors),
    };
    if let Err(errors) = cmd_result {
        error_format::print_errors(None, &errors);
    }
}
