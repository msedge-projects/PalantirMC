//! What Windows reports about the Bedrock packages it has installed.
//!
//! The launcher does not install Bedrock and does not register versions: it
//! asks Windows which packages exist and launches the one the player picks.
//! Reading that answer is all this module does, and it reads *text* rather than
//! touching the machine, so the part with real logic in it is testable on a
//! host that has never seen Bedrock.
//!
//! The reader walks lines on purpose. The command built by
//! [`appx_query_command`] prints one `"key": value` per line, and neither a
//! package identity nor an install path can contain a literal newline, so a
//! line reader is strict enough for the producer this crate starts. A general
//! JSON reader would be needed only if that producer changed -- and the
//! producer is ours.

use std::path::PathBuf;

use crate::paths::{Channel, classify_package};

/// One installed Bedrock package, as the launcher understands it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BedrockInstall {
    /// Which Store channel this package is.
    pub channel: Channel,
    /// The package identity, which is what an activation is addressed by.
    pub package_name: String,
    /// The version Windows reported, empty when it reported none.
    pub version: String,
    /// Where the package is installed, when Windows named a path.
    pub install_path: Option<PathBuf>,
    /// The player-file root that belongs to this install, when one exists.
    ///
    /// The parser leaves this `None`: which layout exists is a fact about the
    /// machine, so the platform module is the one that fills it in.
    pub data_path: Option<PathBuf>,
}

#[derive(Debug, Default)]
struct Partial {
    name: String,
    version: String,
    location: Option<String>,
}

impl Partial {
    /// A package this launcher does not know how to launch is skipped, not
    /// reported as an error: the package list belongs to the player, and one
    /// unfamiliar entry in it is not a failure of the enquiry.
    fn finish(self) -> Option<BedrockInstall> {
        let channel = classify_package(&self.name)?;
        Some(BedrockInstall {
            channel,
            package_name: self.name,
            version: self.version,
            install_path: self.location.map(PathBuf::from),
            data_path: None,
        })
    }
}

/// Read the JSON that `Get-AppxPackage | ConvertTo-Json` prints.
///
/// Both the single-object shape (one match) and the array shape (several) are
/// accepted, because the same command produces either depending on how many
/// packages the machine has.
pub fn parse_get_appx_json(text: &str) -> Result<Vec<BedrockInstall>, String> {
    let mut installs = Vec::new();
    let mut current: Option<Partial> = None;
    for line in text.lines() {
        let Some((key, value)) = key_value(line.trim())? else {
            continue;
        };
        match key.as_str() {
            // A new `Name` starts a new object, which is what makes the array
            // and the single-object shapes share one code path.
            "Name" => {
                if let Some(done) = current.take().and_then(Partial::finish) {
                    installs.push(done);
                }
                current = Some(Partial {
                    name: value.unwrap_or_default(),
                    ..Partial::default()
                });
            }
            "Version" => {
                if let Some(partial) = current.as_mut() {
                    partial.version = value.unwrap_or_default();
                }
            }
            "InstallLocation" => {
                if let Some(partial) = current.as_mut() {
                    partial.location = value.filter(|path| !path.is_empty());
                }
            }
            _ => {}
        }
    }
    if let Some(done) = current.take().and_then(Partial::finish) {
        installs.push(done);
    }
    Ok(installs)
}

/// A `"key": value` line reduced to its key and optional decoded value.
///
/// `Ok(None)` means the line is not one of those -- a brace, a bracket, a
/// fragment -- and is skipped. `Err` is reserved for a quoted value that
/// started and never closed, which is the one malformed shape worth refusing
/// rather than guessing at.
fn key_value(line: &str) -> Result<Option<(String, Option<String>)>, String> {
    let Some(rest) = line.strip_prefix('"') else {
        return Ok(None);
    };
    let Some(close) = rest.find("\":") else {
        return Ok(None);
    };
    let key = rest[..close].to_string();
    let after = rest[close + 2..].trim_start();
    if after.is_empty() || after.starts_with("null") {
        return Ok(Some((key, None)));
    }
    let value = match after.strip_prefix('"') {
        Some(quoted) => Some(read_quoted(quoted)?),
        // An unquoted value is a number or a boolean, and none of the three
        // fields asked for is one, so it is reported as absent rather than
        // guessed into a string.
        None => None,
    };
    Ok(Some((key, value)))
}

/// Decode a quoted value, given the text after its opening quote.
fn read_quoted(quoted: &str) -> Result<String, String> {
    let mut out = String::new();
    let mut chars = quoted.chars();
    while let Some(ch) = chars.next() {
        match ch {
            '"' => return Ok(out),
            '\\' => {
                let Some(escaped) = chars.next() else {
                    return Err("a quoted value ended with a bare backslash".to_string());
                };
                match escaped {
                    '"' => out.push('"'),
                    '\\' => out.push('\\'),
                    '/' => out.push('/'),
                    'n' => out.push('\n'),
                    'r' => out.push('\r'),
                    't' => out.push('\t'),
                    // PowerShell escapes the few characters that matter to a
                    // web page, and a Windows path can legitimately contain
                    // them, so they are decoded rather than passed through.
                    'u' => {
                        let digits: String = chars.by_ref().take(4).collect();
                        let code = u32::from_str_radix(&digits, 16)
                            .ok()
                            .and_then(char::from_u32)
                            .ok_or_else(|| format!("`\\u{digits}` is not a character"))?;
                        out.push(code);
                    }
                    // An escape this reader does not know is kept as written:
                    // a path is hard enough to debug without the reader editing
                    // it on the way past.
                    other => {
                        out.push('\\');
                        out.push(other);
                    }
                }
            }
            other => out.push(other),
        }
    }
    Err("a quoted value was never closed".to_string())
}

/// The read-only enquiry that lists the installed Bedrock packages.
///
/// `-NoProfile` and `-NonInteractive` keep a player's shell configuration out
/// of the launcher's answer and stop the enquiry waiting for input. Nothing
/// here installs, registers or removes anything: the filter narrows the list to
/// the two identities this crate can launch, and the command only reads.
pub fn appx_query_command() -> (String, Vec<String>) {
    let filter = [Channel::Release, Channel::Preview]
        .into_iter()
        .map(|channel| format!("$_.Name -like '{}*'", channel.package_prefix()))
        .collect::<Vec<_>>()
        .join(" -or ");
    let script = format!(
        "Get-AppxPackage | Where-Object {{ {filter} }} | \
         Select-Object Name,Version,InstallLocation | ConvertTo-Json"
    );
    (
        "powershell".to_string(),
        vec![
            "-NoProfile".to_string(),
            "-NonInteractive".to_string(),
            "-Command".to_string(),
            script,
        ],
    )
}

/// The shell activation for an installed package.
///
/// The empty argument is `start`'s window-title slot, which has to be present
/// or `start` treats the activation string as the title instead.
///
/// Shell activation starts the game but reports no process id, so a caller
/// cannot yet say which Bedrock window belongs to which launch. That is the
/// documented reason `IApplicationActivationManager::ActivateApplication` --
/// which returns the id -- is the upgrade this stands in for.
pub fn activate_command(package_name: &str) -> (String, Vec<String>) {
    (
        "cmd".to_string(),
        vec![
            "/c".to_string(),
            "start".to_string(),
            String::new(),
            format!("shell:AppsFolder\\{package_name}!App"),
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const ARRAY: &str = r#"[
    {
        "Name":  "Microsoft.MinecraftUWP",
        "Version":  "1.21.120.0",
        "InstallLocation":  "C:\\Program Files\\WindowsApps\\MinecraftUWP_1.21.120.0_x64__8wekyb3d8bbwe"
    },
    {
        "Name":  "Microsoft.WindowsCalculator",
        "Version":  "11.0.0.0",
        "InstallLocation":  "C:\\Program Files\\WindowsApps\\Calculator_11.0.0.0_x64__8wekyb3d8bbwe"
    }
]"#;

    const SINGLE: &str = r#"{
    "Name":  "Microsoft.MinecraftWindowsBeta",
    "Version":  "1.21.130.0",
    "InstallLocation":  "C:\\WindowsApps\\MinecraftWindowsBeta_1.21.130.0"
}"#;

    #[test]
    fn an_array_reports_each_launchable_package() {
        let installs = parse_get_appx_json(ARRAY).unwrap();
        assert_eq!(installs.len(), 1);
        assert_eq!(installs[0].channel, Channel::Release);
        assert_eq!(installs[0].package_name, "Microsoft.MinecraftUWP");
        assert_eq!(installs[0].version, "1.21.120.0");
    }

    #[test]
    fn a_single_object_is_read_the_same_way() {
        let installs = parse_get_appx_json(SINGLE).unwrap();
        assert_eq!(installs.len(), 1);
        assert_eq!(installs[0].channel, Channel::Preview);
        assert_eq!(installs[0].version, "1.21.130.0");
    }

    #[test]
    fn a_package_this_launcher_cannot_launch_is_skipped() {
        assert_eq!(parse_get_appx_json(ARRAY).unwrap().len(), 1);
        let calculator = r#"{
    "Name":  "Microsoft.WindowsCalculator",
    "Version":  "11.0.0.0",
    "InstallLocation":  null
}"#;
        assert!(parse_get_appx_json(calculator).unwrap().is_empty());
    }

    #[test]
    fn blank_fields_become_empty_values_rather_than_errors() {
        let absent = r#"{
    "Name":  "Microsoft.MinecraftUWP",
    "Version":  null,
    "InstallLocation":  null
}"#;
        let installs = parse_get_appx_json(absent).unwrap();
        assert_eq!(installs.len(), 1);
        assert_eq!(installs[0].version, "");
        assert_eq!(installs[0].install_path, None);

        let missing = r#"{
    "Name":  "Microsoft.MinecraftUWP"
}"#;
        let installs = parse_get_appx_json(missing).unwrap();
        assert_eq!(installs.len(), 1);
        assert_eq!(installs[0].version, "");
        assert_eq!(installs[0].install_path, None);
    }

    #[test]
    fn an_install_location_is_decoded_back_to_one_separator() {
        let installs = parse_get_appx_json(SINGLE).unwrap();
        let expected = PathBuf::from(r"C:\WindowsApps\MinecraftWindowsBeta_1.21.130.0");
        assert_eq!(installs[0].install_path, Some(expected));
    }

    #[test]
    fn an_escaped_character_that_matters_to_a_path_survives() {
        let text = r#"{
    "Name":  "Microsoft.MinecraftUWP",
    "InstallLocation":  "C:\\Users\\Ben \u0026 Jerry\\WindowsApps"
}"#;
        let installs = parse_get_appx_json(text).unwrap();
        let expected = PathBuf::from(r"C:\Users\Ben & Jerry\WindowsApps");
        assert_eq!(installs[0].install_path, Some(expected));
    }

    #[test]
    fn an_unterminated_value_is_refused_rather_than_guessed() {
        let broken = "{\n    \"Name\":  \"Microsoft.MinecraftUWP";
        assert!(parse_get_appx_json(broken).is_err());
    }

    #[test]
    fn an_empty_answer_is_no_packages_not_a_failure() {
        assert!(parse_get_appx_json("").unwrap().is_empty());
        assert!(parse_get_appx_json("null").unwrap().is_empty());
    }

    #[test]
    fn the_enquiry_reads_both_channels_and_writes_nothing() {
        let (program, args) = appx_query_command();
        assert_eq!(program, "powershell");
        assert!(args.contains(&"-NoProfile".to_string()));
        assert!(args.contains(&"-NonInteractive".to_string()));
        let script = args.last().unwrap();
        assert!(script.contains("Get-AppxPackage"));
        assert!(script.contains("Microsoft.MinecraftUWP*"));
        assert!(script.contains("Microsoft.MinecraftWindowsBeta*"));
        for forbidden in ["Add-AppxPackage", "Remove-AppxPackage", "Register-"] {
            assert!(!script.contains(forbidden), "enquiry must not {forbidden}");
        }
    }

    #[test]
    fn activation_addresses_the_app_of_the_package_it_is_given() {
        let (program, args) = activate_command("Microsoft.MinecraftUWP_8wekyb3d8bbwe");
        assert_eq!(program, "cmd");
        assert_eq!(args[0], "/c");
        assert_eq!(args[1], "start");
        assert_eq!(args[2], "");
        let target = r"shell:AppsFolder\Microsoft.MinecraftUWP_8wekyb3d8bbwe!App";
        assert_eq!(args[3], target);
    }
}
