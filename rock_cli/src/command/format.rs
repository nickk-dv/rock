use rock_core::error::ErrorComp;

pub struct CommandFormat {
    pub name: String,
    pub args: Vec<String>,
    pub options: Vec<OptionFormat>,
}

pub struct OptionFormat {
    pub name: String,
    pub args: Vec<String>,
}

pub fn parse() -> Result<CommandFormat, ErrorComp> {
    let mut p = FormatParser::new();

    let cmd_name = if let Some(name) = p.peek_arg() {
        name
    } else {
        return Err(ErrorComp::message(
            "command name is missing, use `rock help` to learn the usage",
        ));
    };

    let mut cmd_args = Vec::new();
    while let Some(arg) = p.peek_arg() {
        cmd_args.push(arg);
    }

    let mut cmd_options = Vec::new();
    while let Some(name) = p.peek_option() {
        let opt_name = name;

        let mut opt_args = Vec::new();
        while let Some(arg) = p.peek_arg() {
            opt_args.push(arg);
        }

        cmd_options.push(OptionFormat {
            name: opt_name,
            args: opt_args,
        })
    }

    Ok(CommandFormat {
        name: cmd_name,
        args: cmd_args,
        options: cmd_options,
    })
}

struct FormatParser {
    args: Vec<String>,
    cursor: usize,
}

impl FormatParser {
    fn new() -> FormatParser {
        FormatParser {
            args: std::env::args().skip(1).collect(),
            cursor: 0,
        }
    }

    fn peek_arg(&mut self) -> Option<String> {
        match self.args.get(self.cursor) {
            Some(arg) => {
                if arg.starts_with("-") || arg.starts_with("--") {
                    None
                } else {
                    self.cursor += 1;
                    Some(arg.clone())
                }
            }
            None => None,
        }
    }

    fn peek_option(&mut self) -> Option<String> {
        match self.args.get(self.cursor) {
            Some(arg) => {
                if let Some(option) = arg.strip_prefix("-") {
                    self.cursor += 1;
                    Some(option.to_string())
                } else if let Some(option) = arg.strip_prefix("--") {
                    self.cursor += 1;
                    Some(option.to_string())
                } else {
                    None
                }
            }
            None => None,
        }
    }
}
