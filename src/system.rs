use std::process::Command;

#[derive(Debug, Clone, Default)]
pub struct SystemReport {
    pub gpu: Check,
    pub wsl: Check,
    pub docker: Check,
}

#[derive(Debug, Clone, Default)]
pub struct Check {
    pub ok: bool,
    pub detail: String,
}

pub fn inspect() -> SystemReport {
    SystemReport {
        gpu: run(
            "nvidia-smi",
            &[
                "--query-gpu=name,memory.total,driver_version",
                "--format=csv,noheader,nounits",
            ],
        ),
        wsl: run("wsl.exe", &["--status"]),
        docker: run("docker", &["--version"]),
    }
}

fn run(program: &str, args: &[&str]) -> Check {
    match Command::new(program).args(args).output() {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
            let detail = if stdout.is_empty() { stderr } else { stdout };
            Check {
                ok: output.status.success(),
                detail: if detail.is_empty() {
                    format!("{program} returned no output")
                } else {
                    detail
                },
            }
        }
        Err(error) => Check {
            ok: false,
            detail: format!("{program} not available: {error}"),
        },
    }
}
