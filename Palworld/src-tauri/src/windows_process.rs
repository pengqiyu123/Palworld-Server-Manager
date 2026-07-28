use std::process::Command;

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

/// Prevent short-lived Windows utilities such as `tasklist` and PowerShell
/// probes from creating a visible console behind the desktop application.
pub fn hidden_command(program: &str) -> Command {
    let mut command = Command::new(program);
    #[cfg(target_os = "windows")]
    command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    command
}

#[cfg(all(test, target_os = "windows"))]
mod tests {
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    #[test]
    fn hidden_command_sets_the_no_console_flag() {
        let source = include_str!("windows_process.rs");
        assert!(source.contains("command.creation_flags(0x0800_0000)"));
        assert_eq!(CREATE_NO_WINDOW, 0x0800_0000);
    }
}
