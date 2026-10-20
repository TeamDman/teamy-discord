//! Schema-driven safety boundary for Figue: values remain opaque; help never starts a command.
use crate::cli_input_error::CliInputError;
macro_rules! named {
    ($levels:expr, $token:expr) => {{
        let token: &str = $token;
        let name = token.split('=').next().unwrap_or(token);
        $levels.iter().rev().find_map(|level| {
            if let Some(long) = name.strip_prefix("--") {
                let long = long.strip_prefix("no-").unwrap_or(long);
                level.args().get(long).map(|(_, arg)| arg)
            } else if name.len() == 2 && name.starts_with('-') {
                level
                    .args()
                    .iter()
                    .find(|(_, arg)| arg.kind().short() == name.chars().nth(1))
                    .map(|(_, arg)| arg)
            } else {
                None
            }
        })
    }};
}
fn validate_value(
    name: &str,
    identifier: &str,
    variants: Option<&[String]>,
    value: &str,
) -> eyre::Result<()> {
    macro_rules! valid_integer {
        ($kind:ty) => {
            value.parse::<$kind>().is_ok()
        };
    }
    let valid = match identifier {
        "u8" => valid_integer!(u8),
        "u16" => valid_integer!(u16),
        "u32" => valid_integer!(u32),
        "u64" => valid_integer!(u64),
        "u128" => valid_integer!(u128),
        "usize" => valid_integer!(usize),
        "i8" => valid_integer!(i8),
        "i16" => valid_integer!(i16),
        "i32" => valid_integer!(i32),
        "i64" => valid_integer!(i64),
        "i128" => valid_integer!(i128),
        "isize" => valid_integer!(isize),
        _ => variants.is_none_or(|allowed| allowed.iter().any(|v| v == value)),
    };
    eyre::ensure!(
        valid,
        CliInputError::InvalidValueFor(name.replace('_', "-"))
    );
    Ok(())
}
fn help(path: &[String]) -> Vec<String> {
    let mut result = path.to_vec();
    result.push("--help".into());
    result
}
fn help_request<T: facet::Facet<'static>>(args: &[String]) -> eyre::Result<Option<Vec<String>>> {
    let schema = figue::builder::<T>()?.build().schema;
    // A standalone help flag before -- wins over validation. Traverse only the
    // verified command prefix; once an unknown token appears, freeze that context.
    let mut help_levels = vec![schema.args()];
    let mut help_path = Vec::new();
    let mut help_index = 0;
    let mut frozen = false;
    while help_index < args.len() {
        let token = &args[help_index];
        if token == "--" {
            break;
        }
        if matches!(token.as_str(), "--help" | "-h") {
            return Ok(Some(help(&help_path)));
        }
        if !frozen {
            if let Some(arg) = named!(help_levels, token) {
                if !arg.value().is_bool()
                    && !arg.kind().is_counted()
                    && !token.contains('=')
                    && args
                        .get(help_index + 1)
                        .is_some_and(|next| !next.starts_with('-'))
                {
                    help_index += 1;
                }
            } else if let Some(command) = help_levels
                .last()
                .ok_or(CliInputError::InvalidValue)?
                .subcommands()
                .values()
                .find(|command| command.cli_name() == token)
            {
                help_path.push(command.cli_name().to_owned());
                help_levels.push(command.args());
            } else {
                frozen = true;
            }
        }
        help_index += 1;
    }
    Ok(None)
}

/// Preserve ordinary hyphen-leading values and negatives. Flag-shaped values use `--option=value`.
/// # Errors
/// Returns only reviewed errors; no diagnostic contains supplied values.
pub fn prepare<T: facet::Facet<'static>>(
    args: impl IntoIterator<Item = std::ffi::OsString>,
) -> eyre::Result<Vec<String>> {
    let schema = figue::builder::<T>()?.build().schema;
    let args: Vec<String> = args
        .into_iter()
        .map(|arg| {
            arg.into_string()
                .map_err(|_redacted_value| CliInputError::InvalidEncoding)
        })
        .collect::<Result<_, _>>()?;
    if let Some(request) = help_request::<T>(&args)? {
        return Ok(request);
    }
    let mut levels = vec![schema.args()];
    let mut path = Vec::new();
    let mut output = Vec::new();
    let mut index = 0;
    while index < args.len() {
        let token = &args[index];
        if token == "--" {
            output.extend_from_slice(&args[index..]);
            break;
        }
        if matches!(token.as_str(), "--help" | "-h") {
            return Ok(help(&path));
        }
        if let Some(arg) = named!(levels, token) {
            if let Some((_, value)) = token.split_once('=') {
                validate_value(
                    arg.name(),
                    arg.value().type_identifier(),
                    arg.value().inner_if_option().enum_variants(),
                    value,
                )?;
            }
            output.push(token.clone());
            if !arg.value().is_bool() && !arg.kind().is_counted() && !token.contains('=') {
                let next = args
                    .get(index + 1)
                    .ok_or_else(|| CliInputError::MissingValue(arg.name().replace('_', "-")))?;
                if matches!(next.as_str(), "--help" | "-h") {
                    return Ok(help(&path));
                }
                if next.starts_with("--") || named!(levels, next).is_some() {
                    return Err(CliInputError::MissingValue(arg.name().replace('_', "-")).into());
                }
                validate_value(
                    arg.name(),
                    arg.value().type_identifier(),
                    arg.value().inner_if_option().enum_variants(),
                    next,
                )?;
                output.push(next.clone());
                index += 1;
            }
        } else if token.starts_with('-') {
            // Short bool clusters are delegated to Figue; verify every letter is a known bool.
            let cluster = token
                .strip_prefix('-')
                .filter(|s| !s.is_empty() && !s.starts_with('-'));
            if !cluster.is_some_and(|s| {
                s.chars().all(|c| {
                    named!(levels, &format!("-{c}"))
                        .is_some_and(|a| a.value().is_bool() || a.kind().is_counted())
                })
            }) {
                // Only reviewed public option spellings may be echoed. All arbitrary
                // labels, inline payloads and values remain redacted.
                let option = match token.split('=').next().unwrap_or_default() {
                    "--mentions" => Some("--mentions"),
                    "--scan-limit" => Some("--scan-limit"),
                    "--token" => Some("--token"),
                    _ => None,
                };
                return Err(CliInputError::UnknownArgument {
                    option,
                    context: path.join(" "),
                }
                .into());
            }
            output.push(token.clone());
        } else {
            let current = *levels.last().ok_or(CliInputError::InvalidValue)?;
            if let Some(command) = current
                .subcommands()
                .values()
                .find(|c| c.cli_name() == token)
            {
                path.push(command.cli_name().to_owned());
                levels.push(command.args());
            } else if current.has_subcommands()
                && !current.args().iter().any(|(_, a)| a.kind().is_positional())
            {
                if path.is_empty() && token == "bot-token" {
                    return Err(CliInputError::RemovedBotTokenCommand.into());
                }
                return Err(CliInputError::InvalidCommand.into());
            }
            output.push(token.clone());
        }
        index += 1;
    }
    Ok(output)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[derive(facet::Facet)]
    struct Args {
        #[facet(figue::named)]
        value: Option<String>,
        #[facet(flatten)]
        builtins: figue::FigueBuiltins,
    }
    fn run(args: &[&str]) -> eyre::Result<Vec<String>> {
        prepare::<Args>(args.iter().map(std::ffi::OsString::from))
    }
    #[test]
    fn missing_value_is_specific_and_redacted() {
        assert_eq!(
            run(&["--value"]).unwrap_err().to_string(),
            "Option --value requires a value; use --help"
        );
        assert!(
            !run(&["--PRIVATE_SECRET"])
                .unwrap_err()
                .to_string()
                .contains("PRIVATE_SECRET")
        );
    }
    #[test]
    fn help_is_not_swallowed_and_explicit_values_stay_values() {
        assert_eq!(run(&["--value", "--help"]).unwrap(), ["--help"]);
        assert_eq!(run(&["--value=--help"]).unwrap(), ["--value=--help"]);
        assert_eq!(
            run(&["--value", "-custom"]).unwrap(),
            ["--value", "-custom"]
        );
        assert_eq!(run(&["--value", "-42"]).unwrap(), ["--value", "-42"]);
        assert_eq!(run(&["--", "--help"]).unwrap(), ["--", "--help"]);
    }
}
