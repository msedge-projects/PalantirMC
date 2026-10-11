//! The two operations that have to touch Windows itself.
//!
//! Enquiry is a read-only shell query and launching is a shell activation, so
//! this module stays a thin wrapper over `std::process` rather than binding the
//! AppX APIs directly: a launcher may ask Windows what is installed and ask it
//! to start something, but not install, register, patch or re-sign a package it
//! does not own. Launching reports no process id, which is the documented
//! reason `IApplicationActivationManager::ActivateApplication` is the upgrade
//! for the slice that needs Bedrock's running state.

use crate::install::BedrockInstall;
#[cfg(windows)]
use crate::install::{activate_command, appx_query_command, parse_get_appx_json};
#[cfg(windows)]
use crate::paths::{data_locations, existing_data_root};

/// The Bedrock packages installed for this user, newest fields and all.
#[cfg(windows)]
pub fn detect_installs() -> Result<Vec<BedrockInstall>, String> {
    let (program, args) = appx_query_command();
    let output = std::process::Command::new(program)
        .args(args)
        .output()
        .map_err(|error| format!("Windows package enquiry could not run: {error}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Windows package enquiry failed: {}", stderr.trim()));
    }
    let mut installs = parse_get_appx_json(&String::from_utf8_lossy(&output.stdout))?;
    for install in &mut installs {
        install.data_path = match install.channel {
            crate::paths::Channel::Release => data_root_from_environment(),
            // The preview client may keep its own player files, and which
            // folder those are is not established yet, so it is left unnamed
            // rather than pointed at the release folder by assumption.
            crate::paths::Channel::Preview => None,
        };
    }
    Ok(installs)
}

/// The player-file root this machine actually uses, from the two profile roots.
#[cfg(windows)]
fn data_root_from_environment() -> Option<std::path::PathBuf> {
    let appdata = std::env::var_os("APPDATA")?;
    let localappdata = std::env::var_os("LOCALAPPDATA")?;
    let roaming = std::path::Path::new(&appdata);
    let local = std::path::Path::new(&localappdata);
    let locations = data_locations(roaming, local);
    existing_data_root(&locations)
}

/// Start an installed package through Windows shell activation.
#[cfg(windows)]
pub fn launch(install: &BedrockInstall) -> Result<(), String> {
    let (program, args) = activate_command(&install.package_name);
    let status = std::process::Command::new(program)
        .args(args)
        .status()
        .map_err(|error| format!("Could not start {}: {error}", install.channel))?;
    if !status.success() {
        let code = status.code();
        return Err(format!(
            "Windows refused to start {}: {code:?}",
            install.channel
        ));
    }
    Ok(())
}

/// Bedrock is a Windows package, so on any other host there is nothing to find.
#[cfg(not(windows))]
pub fn detect_installs() -> Result<Vec<BedrockInstall>, String> {
    Err(
        "Bedrock runs on Windows only, and this machine reports another platform, \
         so no installed copy can be detected."
            .to_string(),
    )
}

/// Bedrock is a Windows package, so on any other host there is nothing to start.
#[cfg(not(windows))]
pub fn launch(_install: &BedrockInstall) -> Result<(), String> {
    Err(
        "Bedrock runs on Windows only, and this machine reports another platform, \
         so it cannot be launched here."
            .to_string(),
    )
}

// The non-Windows stubs are the only behaviour that can be exercised off
// Windows, so their tests are gated the same way: an ungated module would
// import names the Windows build never uses and fail its lint gate.
#[cfg(all(test, not(windows)))]
mod tests {
    use super::{detect_installs, launch};
    use crate::install::BedrockInstall;
    use crate::paths::Channel;

    /// Only its fields are read; nothing is ever launched in these tests.
    fn sample() -> BedrockInstall {
        BedrockInstall {
            channel: Channel::Release,
            package_name: "Microsoft.MinecraftUWP_8wekyb3d8bbwe".to_string(),
            version: String::new(),
            install_path: None,
            data_path: None,
        }
    }

    #[test]
    fn detection_reports_the_other_platform_instead_of_panicking() {
        let error = detect_installs().unwrap_err();
        assert!(error.contains("Windows only"));
    }

    #[test]
    fn launching_reports_the_other_platform_instead_of_panicking() {
        let error = launch(&sample()).unwrap_err();
        assert!(error.contains("Windows only"));
    }
}
