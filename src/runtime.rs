use crate::config::ProjectSettings;
use crate::models::ModelPack;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub const DISTRO: &str = "Ubuntu-24.04";
pub const GSS_ROOT: &str = "/opt/gss";

const EMBEDDED_FILES: &[(&str, &str)] = &[
    (
        "bootstrap_windows.ps1",
        include_str!("../runtime/bootstrap_windows.ps1"),
    ),
    ("bootstrap.sh", include_str!("../runtime/bootstrap.sh")),
    (
        "download_models.py",
        include_str!("../runtime/download_models.py"),
    ),
    ("pipeline.py", include_str!("../runtime/pipeline.py")),
    ("refine.py", include_str!("../runtime/refine.py")),
];

#[derive(Debug, Clone)]
pub struct RuntimeStatus {
    pub wsl_available: bool,
    pub distro_available: bool,
    pub runtime_ready: bool,
    pub cuda_visible: bool,
    pub models_ready: bool,
    pub detail: String,
}

#[derive(Debug, Clone)]
pub struct RuntimeManager {
    pub workspace: PathBuf,
}

impl Default for RuntimeManager {
    fn default() -> Self {
        Self::new(default_workspace())
    }
}

impl RuntimeManager {
    pub fn new(workspace: PathBuf) -> Self {
        Self { workspace }
    }

    pub fn required_directories(&self) -> Vec<PathBuf> {
        vec![
            self.workspace.join("projects"),
            self.workspace.join("runtime"),
            self.workspace.join("logs"),
        ]
    }

    pub fn create_layout(&self) -> std::io::Result<()> {
        for dir in self.required_directories() {
            fs::create_dir_all(dir)?;
        }
        Ok(())
    }

    pub fn materialize_embedded(&self) -> std::io::Result<()> {
        self.create_layout()?;
        let runtime_dir = self.workspace.join("runtime");
        for (name, content) in EMBEDDED_FILES {
            fs::write(runtime_dir.join(name), content)?;
        }
        Ok(())
    }

    pub fn inspect(&self) -> RuntimeStatus {
        let wsl_available = command_success("wsl.exe", &["--status"]);
        let distro_available = if wsl_available {
            let output = Command::new("wsl.exe").args(["--list", "--quiet"]).output();
            output
                .ok()
                .map(|output| {
                    String::from_utf8_lossy(&output.stdout)
                        .replace('\0', "")
                        .lines()
                        .any(|line| line.trim() == DISTRO)
                })
                .unwrap_or(false)
        } else {
            false
        };
        let runtime_ready = distro_available
            && command_success(
                "wsl.exe",
                &[
                    "-d",
                    DISTRO,
                    "-u",
                    "root",
                    "--",
                    "test",
                    "-x",
                    "/opt/gss/venv/bin/python",
                ],
            );
        let cuda_visible = runtime_ready
            && command_success(
                "wsl.exe",
                &["-d", DISTRO, "-u", "root", "--", "nvidia-smi"],
            );
        let models_ready = runtime_ready
            && command_success(
                "wsl.exe",
                &[
                    "-d",
                    DISTRO,
                    "-u",
                    "root",
                    "--",
                    "test",
                    "-f",
                    "/opt/gss/upstream/harmonizer/models/diffusion_harmonizer.pkl",
                ],
            )
            && command_success(
                "wsl.exe",
                &[
                    "-d",
                    DISTRO,
                    "-u",
                    "root",
                    "--",
                    "test",
                    "-f",
                    "/opt/gss/upstream/harmonizer/src/checkpoints/nvidia/Cosmos-Predict2-0.6B-Text2Image/model.pt",
                ],
            );

        let detail = match (
            wsl_available,
            distro_available,
            runtime_ready,
            cuda_visible,
            models_ready,
        ) {
            (false, _, _, _, _) => "WSL is not enabled".to_owned(),
            (_, false, _, _, _) => format!("{DISTRO} is not installed"),
            (_, _, false, _, _) => "GSS Linux runtime is not installed".to_owned(),
            (_, _, _, false, _) => "Runtime exists but CUDA is not visible inside WSL".to_owned(),
            (_, _, _, _, false) => "Runtime ready; Harmonizer model pack not downloaded yet".to_owned(),
            _ => "Local 3DGUT + Harmonizer runtime is ready".to_owned(),
        };

        RuntimeStatus {
            wsl_available,
            distro_available,
            runtime_ready,
            cuda_visible,
            models_ready,
            detail,
        }
    }

    pub fn setup_command(&self) -> Result<(String, Vec<String>), String> {
        self.materialize_embedded()
            .map_err(|error| format!("Could not prepare runtime installer: {error}"))?;
        let script = self.workspace.join("runtime/bootstrap_windows.ps1");
        Ok((
            "powershell.exe".to_owned(),
            vec![
                "-NoProfile".to_owned(),
                "-ExecutionPolicy".to_owned(),
                "Bypass".to_owned(),
                "-File".to_owned(),
                script.display().to_string(),
                "-Distro".to_owned(),
                DISTRO.to_owned(),
            ],
        ))
    }

    pub fn launch_hf_login(&self) -> Result<(), String> {
        let status = Command::new("cmd.exe")
            .args([
                "/C",
                "start",
                "",
                "wsl.exe",
                "-d",
                DISTRO,
                "-u",
                "root",
                "--",
                "/opt/gss/venv/bin/hf",
                "auth",
                "login",
            ])
            .status()
            .map_err(|error| format!("Could not open Hugging Face login terminal: {error}"))?;
        if status.success() {
            Ok(())
        } else {
            Err(format!("Hugging Face login terminal exited with {status}"))
        }
    }

    pub fn model_command(&self, pack: ModelPack) -> Result<(String, Vec<String>), String> {
        self.materialize_embedded()
            .map_err(|error| format!("Could not prepare model downloader: {error}"))?;
        let pack_name = match pack {
            ModelPack::LocalRecommended => "recommended",
            ModelPack::FullHarmonizer => "full",
            ModelPack::NonTemporalJit => {
                return Err(
                    "The optional legacy/JIT checkpoint is not the standalone refinement backend. Select the recommended public Harmonizer pack."
                        .to_owned(),
                )
            }
            ModelPack::FullNuRec => {
                return Err("Full NuRec is a separate >24 GB VRAM backend and is not installed by the local 16 GB model downloader.".to_owned())
            }
        };
        let script = self.windows_to_wsl_path(&self.workspace.join("runtime/download_models.py"))?;
        Ok((
            "wsl.exe".to_owned(),
            vec![
                "-d".to_owned(),
                DISTRO.to_owned(),
                "-u".to_owned(),
                "root".to_owned(),
                "--".to_owned(),
                format!("{GSS_ROOT}/venv/bin/python"),
                script,
                "--pack".to_owned(),
                pack_name.to_owned(),
            ],
        ))
    }

    pub fn pipeline_command(
        &self,
        input_video: &Path,
        output_dir: &Path,
        settings: &ProjectSettings,
    ) -> Result<(String, Vec<String>), String> {
        self.materialize_embedded()
            .map_err(|error| format!("Could not prepare pipeline scripts: {error}"))?;

        let script = self.windows_to_wsl_path(&self.workspace.join("runtime/pipeline.py"))?;
        let video = self.windows_to_wsl_path(input_video)?;
        let output = self.windows_to_wsl_path(output_dir)?;
        let rounds = if settings.refinement.enabled {
            settings.refinement.rounds
        } else {
            0
        };

        let mut args = vec![
            "-d".to_owned(),
            DISTRO.to_owned(),
            "-u".to_owned(),
            "root".to_owned(),
            "--".to_owned(),
            format!("{GSS_ROOT}/venv/bin/python"),
            script,
            "--video".to_owned(),
            video,
            "--output-dir".to_owned(),
            output,
            "--frame-fps".to_owned(),
            settings.capture.frame_fps.to_string(),
            "--blur-threshold".to_owned(),
            settings.capture.blur_threshold.to_string(),
            "--duplicate-threshold".to_owned(),
            settings.capture.duplicate_threshold.to_string(),
            "--max-frames".to_owned(),
            settings.capture.max_frames.to_string(),
            "--matcher".to_owned(),
            settings.capture.matcher.cli_value().to_owned(),
            "--max-steps".to_owned(),
            settings.training.max_steps.to_string(),
            "--cap-max".to_owned(),
            settings.training.quality.cap_max().to_string(),
            "--refinement-rounds".to_owned(),
            rounds.to_string(),
            "--resolution".to_owned(),
            settings.refinement.resolution.to_string(),
            "--distill-steps".to_owned(),
            settings.refinement.distill_steps.to_string(),
            "--max-mean-change".to_owned(),
            settings.refinement.max_mean_change.to_string(),
            "--min-edge-correlation".to_owned(),
            settings.refinement.min_edge_correlation.to_string(),
            "--unit".to_owned(),
            settings.export.unit.clone(),
            "--up-axis".to_owned(),
            settings.export.up_axis.clone(),
            "--scale-multiplier".to_owned(),
            settings.export.scale_multiplier.to_string(),
        ];

        if settings.refinement.temporal {
            args.push("--temporal".to_owned());
        }
        if settings.export.export_spz {
            args.push("--export-spz".to_owned());
        }

        Ok(("wsl.exe".to_owned(), args))
    }

    fn windows_to_wsl_path(&self, path: &Path) -> Result<String, String> {
        let output = Command::new("wsl.exe")
            .args(["-d", DISTRO, "-u", "root", "--", "wslpath", "-a", "-u"])
            .arg(path)
            .output()
            .map_err(|error| format!("Could not invoke WSL path conversion: {error}"))?;
        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).trim().to_owned());
        }
        let translated = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        if translated.is_empty() {
            Err(format!("Could not convert Windows path: {}", path.display()))
        } else {
            Ok(translated)
        }
    }
}

pub fn default_workspace() -> PathBuf {
    env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(env::temp_dir)
        .join("GaussianSplatStudio")
}

fn command_success(program: &str, args: &[&str]) -> bool {
    Command::new(program)
        .args(args)
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_runtime_has_pipeline() {
        assert!(EMBEDDED_FILES.iter().any(|(name, _)| *name == "pipeline.py"));
        assert!(EMBEDDED_FILES.iter().any(|(name, _)| *name == "refine.py"));
    }

    #[test]
    fn default_workspace_has_stable_app_folder() {
        assert_eq!(
            default_workspace().file_name().and_then(|value| value.to_str()),
            Some("GaussianSplatStudio")
        );
    }
}
