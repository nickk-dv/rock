pub const RESET: &str = "\x1B[0m";
pub const RED_BOLD: &str = "\x1B[1;31m";
pub const GREEN_BOLD: &str = "\x1B[1;32m";
pub const YELLOW_BOLD: &str = "\x1B[1;33m";
pub const CYAN: &str = "\x1B[0;36m";
pub const CYAN_BOLD: &str = "\x1B[1;36m";
pub const WHITE_BOLD: &str = "\x1B[1;37m";

pub struct OutputStyle {
    pub color_out: &'static AnsiColors,
    pub color_err: &'static AnsiColors,
}

pub struct AnsiColors {
    pub reset: &'static str,
    pub red_bold: &'static str,
    pub green_bold: &'static str,
    pub yellow_bold: &'static str,
    pub cyan: &'static str,
    pub cyan_bold: &'static str,
    pub white_bold: &'static str,
}

const ANSI_COLORS: AnsiColors = AnsiColors {
    reset: "\x1B[0m",
    red_bold: "\x1B[1;31m",
    green_bold: "\x1B[1;32m",
    yellow_bold: "\x1B[1;33m",
    cyan: "\x1B[0;36m",
    cyan_bold: "\x1B[1;36m",
    white_bold: "\x1B[1;37m",
};

const ANSI_COLORS_EMPTY: AnsiColors = AnsiColors {
    reset: "",
    red_bold: "",
    green_bold: "",
    yellow_bold: "",
    cyan: "",
    cyan_bold: "",
    white_bold: "",
};

impl OutputStyle {
    pub fn new() -> OutputStyle {
        use std::io::IsTerminal;

        OutputStyle {
            color_out: if std::io::stdout().is_terminal() {
                &ANSI_COLORS
            } else {
                &ANSI_COLORS_EMPTY
            },
            color_err: if std::io::stderr().is_terminal() {
                &ANSI_COLORS
            } else {
                &ANSI_COLORS_EMPTY
            },
        }
    }
}
