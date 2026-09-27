use std::process::Command;
use std::path::Path;

#[derive(Debug, Default, Clone)]
pub struct EnvironmentRegisters {
    pub git_branch: String,
    pub git_commit: String,
    pub working_directory: String,
    pub modified_files: Vec<String>,
}

pub fn extract_registers(workspace_dir: &Path) -> EnvironmentRegisters {
    let mut registers = EnvironmentRegisters::default();
    
    registers.working_directory = workspace_dir.to_string_lossy().to_string();

    if let Ok(output) = Command::new("git")
        .arg("rev-parse")
        .arg("--abbrev-ref")
        .arg("HEAD")
        .current_dir(workspace_dir)
        .output() {
        if output.status.success() {
            registers.git_branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
        }
    }

    if let Ok(output) = Command::new("git")
        .arg("rev-parse")
        .arg("HEAD")
        .current_dir(workspace_dir)
        .output() {
        if output.status.success() {
            registers.git_commit = String::from_utf8_lossy(&output.stdout).trim().to_string();
        }
    }

    if let Ok(output) = Command::new("git")
        .arg("diff")
        .arg("--name-only")
        .current_dir(workspace_dir)
        .output() {
        if output.status.success() {
            let files_str = String::from_utf8_lossy(&output.stdout);
            registers.modified_files = files_str
                .lines()
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
                .collect();
        }
    }

    registers
}
