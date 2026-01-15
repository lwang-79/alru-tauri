use std::io::{self, BufRead, BufReader};
use std::process::{Command, Output, Stdio};
use std::sync::mpsc;
use std::thread;
use tauri::Emitter;

/// Helper to create a clean Command with aggressive environment isolation.
/// This prevents parent process environment variables (like NODE_OPTIONS or BUN_INSTALL)
/// from interfering with child processes, especially packaged binaries like Amplify CLI.
pub fn create_clean_command(program: &str) -> Command {
    let mut cmd = Command::new(program);

    // Highly aggressive environment clearing to stop pkg-packaged binaries from being misled
    // We keep only the minimum necessary for the OS and for basic user identification.
    let essentials = [
        "PATH",
        "HOME",
        "USER",
        "LOGNAME",
        "SHELL",
        "LANG",
        "LC_ALL",
        "TERM",
        "PWD",
        "TMPDIR",
        // Windows essentials
        "SystemRoot",
        "SystemDrive",
        "TEMP",
        "TMP",
        "USERNAME",
        "USERPROFILE",
        "COMPUTERNAME",
        "COMSPEC",
        "ProgramData",
        "ProgramFiles",
        "ProgramFiles(x86)",
        "CommonProgramFiles",
        "APPDATA",
        "LOCALAPPDATA",
        "PATHEXT",
    ];
    let vars: Vec<(String, String)> = std::env::vars().collect();

    cmd.env_clear();
    for (key, value) in vars {
        if essentials.iter().any(|&e| e.eq_ignore_ascii_case(&key))
            || key.starts_with("AWS_")
            || key.starts_with("LC_")
        {
            cmd.env(key, value);
        }
    }

    // Explicitly blacklisted variables that specifically break node/pkg in dev environments
    cmd.env_remove("NODE_OPTIONS");
    cmd.env_remove("NODE_PATH");
    cmd.env_remove("ELECTRON_RUN_AS_NODE");
    cmd.env_remove("ELECTRON_NO_ASAR");
    cmd.env_remove("BUN_INSTALL");
    cmd.env_remove("npm_config_prefix");

    cmd
}

/// Helper to create a clean Command that runs via a shell on Windows.
/// This is necessary for executing .cmd/.bat files like npm or amplify.
pub fn create_clean_shell_command(program: &str) -> Command {
    if cfg!(target_os = "windows") {
        let mut cmd = create_clean_command("cmd");
        cmd.arg("/C").arg(program);
        cmd
    } else {
        create_clean_command(program)
    }
}

/// Helper to run a shell command with a clean environment.
pub fn run_clean_sh_c(cmd_str: &str) -> io::Result<Output> {
    if cfg!(target_os = "windows") {
        let mut c = create_clean_command("cmd");
        c.arg("/C").arg(cmd_str);
        c.output()
    } else {
        let mut c = create_clean_command("sh");
        c.arg("-c").arg(cmd_str);
        c.output()
    }
}

/// Trait to extend std::process::Command with cleaning capabilities.
pub trait CommandExtClean {
    fn clean_env(self) -> Command;
}

impl CommandExtClean for Command {
    fn clean_env(mut self) -> Command {
        // Aggressive environment clearing
        let essentials = [
            "PATH",
            "HOME",
            "USER",
            "LOGNAME",
            "SHELL",
            "LANG",
            "LC_ALL",
            "TERM",
            "PWD",
            "TMPDIR",
            // Windows essentials
            "SystemRoot",
            "SystemDrive",
            "TEMP",
            "TMP",
            "USERNAME",
            "USERPROFILE",
            "COMPUTERNAME",
            "COMSPEC",
            "ProgramData",
            "ProgramFiles",
            "ProgramFiles(x86)",
            "CommonProgramFiles",
            "APPDATA",
            "LOCALAPPDATA",
            "PATHEXT",
        ];

        let mut vars_to_keep = Vec::new();
        for (key, value) in std::env::vars() {
            if essentials.iter().any(|&e| e.eq_ignore_ascii_case(&key))
                || key.starts_with("AWS_")
                || key.starts_with("LC_")
            {
                vars_to_keep.push((key, value));
            }
        }

        self.env_clear();
        for (key, value) in vars_to_keep {
            self.env(key, value);
        }

        // Explicitly remove blacklisted variables
        self.env_remove("NODE_OPTIONS");
        self.env_remove("NODE_PATH");
        self.env_remove("ELECTRON_RUN_AS_NODE");
        self.env_remove("ELECTRON_NO_ASAR");
        self.env_remove("BUN_INSTALL");
        self.env_remove("npm_config_prefix");

        self
    }
}

/// Helper to run a command with streaming output via Tauri events.
pub fn run_command_streaming(
    mut cmd: Command,
    window: &tauri::Window,
    event_name: &str,
) -> Result<bool, String> {
    let (tx, rx) = mpsc::channel::<String>();

    let mut child = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to start process: {}", e))?;

    let stdout = child.stdout.take().ok_or("Failed to capture stdout")?;
    let stderr = child.stderr.take().ok_or("Failed to capture stderr")?;

    let tx_stdout = tx.clone();
    let tx_stderr = tx;

    // Thread to read stdout
    thread::spawn(move || {
        let reader = BufReader::new(stdout);
        for line in reader.lines().map_while(Result::ok) {
            let _ = tx_stdout.send(line);
        }
    });

    // Thread to read stderr
    thread::spawn(move || {
        let reader = BufReader::new(stderr);
        for line in reader.lines().map_while(Result::ok) {
            let _ = tx_stderr.send(line);
        }
    });

    // Read output and emit to frontend
    loop {
        match rx.recv_timeout(std::time::Duration::from_millis(100)) {
            Ok(line) => {
                let _ = window.emit(event_name, format!("{}\n", line));
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if let Ok(Some(status)) = child.try_wait() {
                    // Process finished, drain remaining output
                    while let Ok(line) = rx.recv_timeout(std::time::Duration::from_millis(100)) {
                        let _ = window.emit(event_name, format!("{}\n", line));
                    }
                    return Ok(status.success());
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }

    let status = child
        .wait()
        .map_err(|e| format!("Failed to wait for process: {}", e))?;
    Ok(status.success())
}
