mod args;
mod cmd;

pub fn cli() {
    match args::parse_args() {
        Ok(cmd::Command::New(data)) => cmd::new::cmd(data),
        Ok(cmd::Command::Check) => cmd::check::cmd(),
        Ok(cmd::Command::Build(data)) => cmd::build::cmd(data),
        Ok(cmd::Command::Run(data)) => cmd::run::cmd(data),
        Ok(cmd::Command::Help) => cmd::help::cmd(),
        Ok(cmd::Command::Version) => cmd::version::cmd(),
        Err(errors) => {
            let vfs = crate::vfs::Vfs::new();
            crate::error::format::print_errors(&vfs, &errors)
        }
    };
}
