/// Reviewed CLI diagnostics contain schema names, never caller values or argv.
#[derive(Debug)]
pub enum CliInputError {
    MissingValue(String),
    InvalidValueFor(String),
    UnknownArgument {
        option: Option<&'static str>,
        context: String,
    },
    InvalidCommand,
    RemovedBotTokenCommand,
    InvalidEncoding,
    InvalidValue,
}
impl std::fmt::Display for CliInputError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingValue(name) => write!(f, "Option --{name} requires a value; use --help"),
            Self::InvalidValueFor(name) => write!(f, "Invalid value for --{name}; use --help"),
            Self::UnknownArgument { option, context } => {
                let context = if context.is_empty() {
                    "this command"
                } else {
                    context
                };
                if let Some(option) = option {
                    write!(f, "Unknown option {option} for {context}; ")?;
                } else {
                    write!(f, "Unknown argument for {context} (value omitted); ")?;
                }
                if context == "this command" {
                    f.write_str("use --help")
                } else {
                    write!(f, "use {context} --help")
                }
            }
            Self::RemovedBotTokenCommand => f.write_str("The bot-token command was removed; use profile broker set/show/clear for references, or --bot-token/TEAMY_DISCORD_BOT_TOKEN for one-off credentials"),
            Self::InvalidCommand => f.write_str("Unknown command; use --help"),
            Self::InvalidEncoding => f.write_str("Command line contains invalid Unicode"),
            Self::InvalidValue => {
                f.write_str("Invalid value or missing required argument; use --help")
            }
        }
    }
}
impl std::error::Error for CliInputError {}
