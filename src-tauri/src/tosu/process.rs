use std::{
    env,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
};

#[cfg(target_os = "linux")]
use std::{fs::File, io::Read};

use serde::Serialize;
use thiserror::Error;

#[cfg(target_os = "linux")]
const PTRACE_CAPABILITY: &str = "cap_sys_ptrace=eip";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TosuMemoryAccess {
    #[default]
    NotRequired,
    Granted,
    Required,
    Unavailable,
}

#[derive(Debug, Error)]
pub enum MemoryAccessError {
    #[cfg(target_os = "linux")]
    #[error("tosu was not found on PATH")]
    NotInstalled,
    #[cfg(target_os = "linux")]
    #[error("the installed tosu launcher does not expose a native binary")]
    UnsupportedLauncher,
    #[cfg(target_os = "linux")]
    #[error("pkexec and the libcap tools are required to grant memory access")]
    ToolsUnavailable,
    #[cfg(target_os = "linux")]
    #[error("the system authorization prompt was cancelled or denied")]
    AuthorizationDenied,
    #[cfg(target_os = "linux")]
    #[error("could not start the system authorization prompt: {0}")]
    Prompt(#[source] std::io::Error),
    #[cfg(target_os = "linux")]
    #[error("memory access was not present after authorization")]
    VerificationFailed,
    #[cfg(not(target_os = "linux"))]
    #[error("this platform does not require Linux ptrace access")]
    UnsupportedPlatform,
}

#[derive(Debug, Error)]
pub enum ProcessError {
    #[error("tosu was not found on PATH")]
    NotInstalled,
    #[error("Companion already owns a running tosu process")]
    AlreadyRunning,
    #[error("failed to launch tosu: {0}")]
    Launch(#[from] std::io::Error),
    #[error("the running tosu process was not launched by Companion")]
    NotOwned,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProcessOwnership {
    External,
    CompanionOwned,
}

impl ProcessOwnership {
    pub fn may_terminate(self) -> bool {
        matches!(self, Self::CompanionOwned)
    }
}

struct OwnedProcess {
    child: Child,
}

#[derive(Default)]
pub struct TosuProcess {
    configured_path: Option<PathBuf>,
    owned: Option<OwnedProcess>,
}

impl TosuProcess {
    pub fn resolve_executable() -> Option<PathBuf> {
        resolve_from(None)
    }

    pub fn memory_access_status() -> TosuMemoryAccess {
        memory_access_status()
    }

    pub fn grant_memory_access() -> Result<PathBuf, MemoryAccessError> {
        grant_memory_access()
    }

    pub fn launch(&mut self) -> Result<PathBuf, ProcessError> {
        self.refresh();
        if self.owned.is_some() {
            return Err(ProcessError::AlreadyRunning);
        }

        let executable =
            resolve_from(self.configured_path.as_deref()).ok_or(ProcessError::NotInstalled)?;
        let child = Command::new(&executable)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;

        self.owned = Some(OwnedProcess { child });
        Ok(executable)
    }

    pub fn refresh(&mut self) -> bool {
        let exited = self
            .owned
            .as_mut()
            .is_some_and(|owned| owned.child.try_wait().ok().flatten().is_some());
        if exited {
            self.owned = None;
        }
        self.owned.is_some()
    }

    pub fn is_owned(&mut self) -> bool {
        self.refresh()
    }

    fn ownership(&mut self) -> ProcessOwnership {
        if self.refresh() {
            ProcessOwnership::CompanionOwned
        } else {
            ProcessOwnership::External
        }
    }

    pub fn stop_owned(&mut self) -> Result<(), ProcessError> {
        if !self.ownership().may_terminate() {
            return Err(ProcessError::NotOwned);
        }
        let Some(mut owned) = self.owned.take() else {
            return Err(ProcessError::NotOwned);
        };
        owned.child.kill().map_err(ProcessError::Launch)?;
        let _ = owned.child.wait();
        Ok(())
    }

    pub fn stop_owned_on_shutdown(&mut self) {
        if let Some(mut owned) = self.owned.take() {
            let _ = owned.child.kill();
            let _ = owned.child.wait();
        }
    }
}

fn resolve_from(configured: Option<&Path>) -> Option<PathBuf> {
    if let Some(path) = configured.filter(|path| path.is_file()) {
        return Some(path.to_path_buf());
    }

    let names: &[&str] = if cfg!(windows) {
        &["tosu.exe", "tosu"]
    } else {
        &["tosu"]
    };
    env::var_os("PATH")
        .into_iter()
        .flat_map(|path| env::split_paths(&path).collect::<Vec<_>>())
        .flat_map(|directory| names.iter().map(move |name| directory.join(name)))
        .find(|candidate| candidate.is_file())
}

#[cfg(target_os = "linux")]
fn memory_access_status() -> TosuMemoryAccess {
    let Some(target) = resolve_capability_target() else {
        return TosuMemoryAccess::Unavailable;
    };
    let Some(getcap) = resolve_trusted_tool("getcap") else {
        return TosuMemoryAccess::Unavailable;
    };
    match Command::new(getcap).arg(target).output() {
        Ok(output) if output.status.success() => {
            if has_effective_ptrace_capability(&String::from_utf8_lossy(&output.stdout)) {
                TosuMemoryAccess::Granted
            } else {
                TosuMemoryAccess::Required
            }
        }
        _ => TosuMemoryAccess::Unavailable,
    }
}

#[cfg(not(target_os = "linux"))]
fn memory_access_status() -> TosuMemoryAccess {
    TosuMemoryAccess::NotRequired
}

#[cfg(target_os = "linux")]
fn grant_memory_access() -> Result<PathBuf, MemoryAccessError> {
    let target = resolve_capability_target().ok_or_else(|| {
        if TosuProcess::resolve_executable().is_some() {
            MemoryAccessError::UnsupportedLauncher
        } else {
            MemoryAccessError::NotInstalled
        }
    })?;
    let pkexec = resolve_trusted_tool("pkexec").ok_or(MemoryAccessError::ToolsUnavailable)?;
    let setcap = resolve_trusted_tool("setcap").ok_or(MemoryAccessError::ToolsUnavailable)?;

    let status = Command::new(pkexec)
        .arg(setcap)
        .arg(PTRACE_CAPABILITY)
        .arg(&target)
        .status()
        .map_err(MemoryAccessError::Prompt)?;
    if !status.success() {
        return Err(MemoryAccessError::AuthorizationDenied);
    }
    if memory_access_status() != TosuMemoryAccess::Granted {
        return Err(MemoryAccessError::VerificationFailed);
    }
    Ok(target)
}

#[cfg(not(target_os = "linux"))]
fn grant_memory_access() -> Result<PathBuf, MemoryAccessError> {
    Err(MemoryAccessError::UnsupportedPlatform)
}

#[cfg(target_os = "linux")]
fn resolve_capability_target() -> Option<PathBuf> {
    let launcher = TosuProcess::resolve_executable()?.canonicalize().ok()?;
    if is_elf_binary(&launcher) {
        return Some(launcher);
    }

    let mut file = File::open(&launcher).ok()?;
    let mut bytes = vec![0_u8; 8 * 1024];
    let read = file.read(&mut bytes).ok()?;
    bytes.truncate(read);
    let script = std::str::from_utf8(&bytes).ok()?;
    let target = parse_wrapper_exec_target(script)?;
    let canonical = target.canonicalize().ok()?;
    is_elf_binary(&canonical).then_some(canonical)
}

#[cfg(target_os = "linux")]
fn is_elf_binary(path: &Path) -> bool {
    let mut magic = [0_u8; 4];
    File::open(path)
        .and_then(|mut file| file.read_exact(&mut magic))
        .is_ok()
        && magic == *b"\x7fELF"
}

#[cfg(target_os = "linux")]
fn parse_wrapper_exec_target(script: &str) -> Option<PathBuf> {
    script.lines().find_map(|line| {
        let command = line.trim().strip_prefix("exec ")?.trim_start();
        let token = command.split_whitespace().next()?.trim_matches(['\'', '"']);
        let path = PathBuf::from(token);
        path.is_absolute().then_some(path)
    })
}

#[cfg(target_os = "linux")]
fn has_effective_ptrace_capability(output: &str) -> bool {
    output.split_whitespace().skip(1).any(|capabilities| {
        capabilities.split(',').any(|capability| {
            let Some((name, flags)) = capability.split_once(['=', '+']) else {
                return false;
            };
            name == "cap_sys_ptrace" && flags.contains('e')
        })
    })
}

#[cfg(target_os = "linux")]
fn resolve_trusted_tool(name: &str) -> Option<PathBuf> {
    use std::os::unix::fs::MetadataExt;

    ["/usr/bin", "/usr/sbin", "/bin", "/sbin"]
        .into_iter()
        .map(|directory| Path::new(directory).join(name))
        .filter_map(|candidate| candidate.canonicalize().ok())
        .find(|candidate| {
            candidate.metadata().is_ok_and(|metadata| {
                metadata.is_file() && metadata.uid() == 0 && metadata.mode() & 0o022 == 0
            })
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_companion_owned_processes_may_be_terminated() {
        assert!(ProcessOwnership::CompanionOwned.may_terminate());
        assert!(!ProcessOwnership::External.may_terminate());
    }

    #[test]
    fn missing_configured_path_falls_back_without_claiming_ownership() {
        let mut process = TosuProcess {
            configured_path: Some(PathBuf::from("/path/that/does/not/exist")),
            ..TosuProcess::default()
        };
        assert!(!process.is_owned());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn resolves_an_absolute_native_target_from_a_shell_wrapper() {
        let script = r#"#!/bin/bash
exec /opt/tosu/tosu --update=false "$@"
"#;
        assert_eq!(
            parse_wrapper_exec_target(script),
            Some(PathBuf::from("/opt/tosu/tosu"))
        );
        assert_eq!(parse_wrapper_exec_target("exec tosu --flag"), None);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn requires_an_effective_ptrace_capability() {
        assert!(has_effective_ptrace_capability(
            "/opt/tosu/tosu cap_sys_ptrace=eip\n"
        ));
        assert!(has_effective_ptrace_capability(
            "/opt/tosu/tosu cap_sys_ptrace+ep\n"
        ));
        assert!(!has_effective_ptrace_capability("/opt/tosu/tosu\n"));
        assert!(!has_effective_ptrace_capability(
            "/opt/tosu/tosu cap_net_bind_service=ep\n"
        ));
    }
}
