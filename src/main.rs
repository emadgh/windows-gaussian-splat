mod config;
mod models;
mod pipeline;
mod project;
mod runner;
mod runtime;
mod system;

use config::{MatcherMode, ProjectSettings, QualityPreset};
use eframe::egui;
use models::ModelPack;
use pipeline::RuntimeProfile;
use project::ProjectManifest;
use runner::{JobEvent, JobRunner};
use runtime::{RuntimeManager, RuntimeStatus};
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::time::Duration;
use system::{Check, SystemReport};

fn main() -> eframe::Result {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1040.0, 860.0])
            .with_min_inner_size([800.0, 620.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Gaussian Splat Studio",
        native_options,
        Box::new(|_cc| Ok(Box::new(StudioApp::default()))),
    )
}

struct StudioApp {
    profile: RuntimeProfile,
    model_pack: ModelPack,
    input_video: String,
    output_dir: String,
    active_output: Option<PathBuf>,
    report: Option<SystemReport>,
    runtime_status: Option<RuntimeStatus>,
    runtime: RuntimeManager,
    settings: ProjectSettings,
    runner: JobRunner,
    stage: String,
    progress: f32,
    logs: VecDeque<String>,
    notice: Option<String>,
}

impl Default for StudioApp {
    fn default() -> Self {
        let runtime = RuntimeManager::default();
        Self {
            profile: RuntimeProfile::Local16Gb,
            model_pack: ModelPack::LocalRecommended,
            input_video: String::new(),
            output_dir: String::new(),
            active_output: None,
            report: None,
            runtime_status: Some(runtime.inspect()),
            runtime,
            settings: ProjectSettings::default(),
            runner: JobRunner::default(),
            stage: "idle".to_owned(),
            progress: 0.0,
            logs: VecDeque::new(),
            notice: None,
        }
    }
}

impl eframe::App for StudioApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.consume_events();
        if self.runner.is_running() {
            ui.ctx().request_repaint_after(Duration::from_millis(100));
        }

        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.heading("Gaussian Splat Studio");
            ui.label("Video → COLMAP → 3DGUT → NVIDIA Harmonizer → offline distillation → PLY/SPZ");
            ui.add_space(8.0);

            if let Some(notice) = &self.notice {
                ui.label(notice);
                ui.add_space(6.0);
            }

            self.system_ui(ui);
            ui.add_space(8.0);
            self.runtime_ui(ui);
            ui.add_space(8.0);
            self.capture_ui(ui);
            ui.add_space(8.0);
            self.quality_ui(ui);
            ui.add_space(8.0);
            self.run_ui(ui);
            ui.add_space(8.0);
            self.log_ui(ui);
            ui.add_space(8.0);

            ui.collapsing("Pipeline details", |ui| {
                for (index, step) in pipeline::plan(self.profile).iter().enumerate() {
                    ui.strong(format!("{}. {}", index + 1, step.name));
                    ui.label(step.detail);
                    ui.add_space(3.0);
                }
            });
        });
    }
}

impl StudioApp {
    fn system_ui(&mut self, ui: &mut egui::Ui) {
        ui.group(|ui| {
            ui.heading("1. System & runtime");
            ui.horizontal_wrapped(|ui| {
                if ui.button("Refresh checks").clicked() {
                    self.report = Some(system::inspect());
                    self.runtime_status = Some(self.runtime.inspect());
                }
                ui.label(format!("Workspace: {}", self.runtime.workspace.display()));
            });

            if let Some(report) = &self.report {
                check_row(ui, "Windows NVIDIA GPU", &report.gpu);
                check_row(ui, "WSL", &report.wsl);
            }
            if let Some(status) = &self.runtime_status {
                status_row(ui, "Ubuntu 24.04", status.distro_available);
                status_row(ui, "GSS runtime", status.runtime_ready);
                status_row(ui, "CUDA inside WSL", status.cuda_visible);
                status_row(ui, "Harmonizer models", status.models_ready);
                ui.small(&status.detail);
            }

            ui.horizontal_wrapped(|ui| {
                let busy = self.runner.is_running();
                if ui
                    .add_enabled(!busy, egui::Button::new("Install / Repair Runtime"))
                    .clicked()
                {
                    match self.runtime.setup_command() {
                        Ok((program, args)) => self.start_task(program, args, "runtime setup"),
                        Err(error) => self.notice = Some(error),
                    }
                }
                if ui
                    .add_enabled(!busy, egui::Button::new("Hugging Face Login"))
                    .clicked()
                {
                    match self.runtime.launch_hf_login() {
                        Ok(()) => {
                            self.notice = Some(
                                "A WSL terminal was opened for 'hf auth login'. Accept the NVIDIA Cosmos model terms in your browser/account before downloading models."
                                    .to_owned(),
                            )
                        }
                        Err(error) => self.notice = Some(error),
                    }
                }
            });
        });
    }

    fn runtime_ui(&mut self, ui: &mut egui::Ui) {
        ui.group(|ui| {
            ui.heading("2. Reconstruction backend & models");
            ui.radio_value(
                &mut self.profile,
                RuntimeProfile::Local16Gb,
                RuntimeProfile::Local16Gb.label(),
            );
            ui.radio_value(
                &mut self.profile,
                RuntimeProfile::FullNuRec,
                RuntimeProfile::FullNuRec.label(),
            );

            if self.profile == RuntimeProfile::FullNuRec {
                ui.label(
                    "Full NuRec is hardware-gated (>24 GB VRAM, 48 GB+ recommended). This 16 GB workstation uses the local 3DGUT + Harmonizer backend; NuRec remains a separate host backend.",
                );
            }

            egui::ComboBox::from_id_salt("model_pack")
                .selected_text(self.model_pack.info().title)
                .show_ui(ui, |ui| {
                    for pack in ModelPack::ALL {
                        ui.selectable_value(&mut self.model_pack, pack, pack.info().title);
                    }
                });
            let info = self.model_pack.info();
            ui.label(format!(
                "Approx. model payload: {:.2} GB; recommended free disk: {} GB",
                info.payload_download_gb, info.free_disk_gb
            ));
            ui.small(info.description);

            if ui
                .add_enabled(!self.runner.is_running(), egui::Button::new("Download Model Pack"))
                .clicked()
            {
                match self.runtime.model_command(self.model_pack) {
                    Ok((program, args)) => self.start_task(program, args, "model download"),
                    Err(error) => self.notice = Some(error),
                }
            }
        });
    }

    fn capture_ui(&mut self, ui: &mut egui::Ui) {
        ui.group(|ui| {
            ui.heading("3. Capture");
            ui.horizontal(|ui| {
                if ui.button("Choose video").clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("Video", &["mp4", "mov", "mkv", "avi", "m4v"])
                        .pick_file()
                    {
                        self.input_video = path.display().to_string();
                    }
                }
                ui.monospace(if self.input_video.is_empty() {
                    "No input selected"
                } else {
                    &self.input_video
                });
            });
            ui.horizontal(|ui| {
                if ui.button("Choose project folder").clicked() {
                    if let Some(path) = rfd::FileDialog::new().pick_folder() {
                        self.output_dir = path.display().to_string();
                    }
                }
                ui.monospace(if self.output_dir.is_empty() {
                    "No project folder selected"
                } else {
                    &self.output_dir
                });
            });

            ui.horizontal_wrapped(|ui| {
                ui.label("Extract FPS");
                ui.add(egui::DragValue::new(&mut self.settings.capture.frame_fps).range(0.5..=10.0).speed(0.25));
                ui.label("Blur threshold");
                ui.add(egui::DragValue::new(&mut self.settings.capture.blur_threshold).range(0.0..=500.0).speed(2.0));
                ui.label("Max frames");
                ui.add(egui::DragValue::new(&mut self.settings.capture.max_frames).range(100..=4000));
            });
            ui.horizontal(|ui| {
                ui.label("Matcher");
                egui::ComboBox::from_id_salt("matcher")
                    .selected_text(self.settings.capture.matcher.label())
                    .show_ui(ui, |ui| {
                        for matcher in MatcherMode::ALL {
                            ui.selectable_value(
                                &mut self.settings.capture.matcher,
                                matcher,
                                matcher.label(),
                            );
                        }
                    });
                ui.small("Auto uses exhaustive matching for smaller captures and sequential matching for larger video scans.");
            });
        });
    }

    fn quality_ui(&mut self, ui: &mut egui::Ui) {
        ui.group(|ui| {
            ui.heading("4. 3DGUT, Harmonizer & export");
            ui.horizontal(|ui| {
                ui.label("3DGUT quality");
                egui::ComboBox::from_id_salt("quality")
                    .selected_text(self.settings.training.quality.label())
                    .show_ui(ui, |ui| {
                        for quality in QualityPreset::ALL {
                            ui.selectable_value(
                                &mut self.settings.training.quality,
                                quality,
                                quality.label(),
                            );
                        }
                    });
                ui.label("Steps");
                ui.add(egui::DragValue::new(&mut self.settings.training.max_steps).range(7_000..=60_000).speed(500));
            });

            ui.checkbox(&mut self.settings.refinement.enabled, "Run Harmonizer offline refinement");
            ui.add_enabled_ui(self.settings.refinement.enabled, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label("Rounds");
                    ui.add(egui::DragValue::new(&mut self.settings.refinement.rounds).range(1..=3));
                    ui.checkbox(&mut self.settings.refinement.temporal, "Temporal Harmonizer");
                    ui.label("Resolution");
                    egui::ComboBox::from_id_salt("harmonizer_resolution")
                        .selected_text(self.settings.refinement.resolution.to_string())
                        .show_ui(ui, |ui| {
                            for resolution in [960_u32, 1024, 1360] {
                                ui.selectable_value(
                                    &mut self.settings.refinement.resolution,
                                    resolution,
                                    resolution.to_string(),
                                );
                            }
                        });
                });
                ui.horizontal(|ui| {
                    ui.label("Distill steps / round");
                    ui.add(egui::DragValue::new(&mut self.settings.refinement.distill_steps).range(250..=2500).speed(50));
                    ui.small("Temporal mode automatically falls back to non-temporal if it cannot run on the local GPU.");
                });
            });

            ui.horizontal_wrapped(|ui| {
                ui.checkbox(&mut self.settings.export.export_spz, "Also export SPZ");
                ui.label("Scale multiplier");
                ui.add(egui::DragValue::new(&mut self.settings.export.scale_multiplier).range(0.000001..=1_000_000.0).speed(0.01));
                ui.label("Unit");
                ui.text_edit_singleline(&mut self.settings.export.unit);
                ui.label("Up axis metadata");
                ui.text_edit_singleline(&mut self.settings.export.up_axis);
            });
            ui.small("Raw PLY is always preserved. refined.ply is the primary 3ds Max 2027.2+ interchange asset; SPZ is optional compact output.");
        });
    }

    fn run_ui(&mut self, ui: &mut egui::Ui) {
        ui.group(|ui| {
            ui.heading("5. Run");
            let local_profile = self.profile == RuntimeProfile::Local16Gb;
            let paths_ready = !self.input_video.is_empty() && !self.output_dir.is_empty();
            let runtime_ready = self
                .runtime_status
                .as_ref()
                .map(|status| status.runtime_ready && status.cuda_visible && status.models_ready)
                .unwrap_or(false);
            let can_start = local_profile && paths_ready && runtime_ready && !self.runner.is_running();

            ui.horizontal(|ui| {
                if ui
                    .add_enabled(can_start, egui::Button::new("Start Reconstruction"))
                    .clicked()
                {
                    self.start_reconstruction();
                }
                if ui
                    .add_enabled(self.runner.is_running(), egui::Button::new("Cancel"))
                    .clicked()
                {
                    self.runner.cancel();
                }
                ui.strong(format!("Stage: {}", self.stage));
            });
            ui.add(egui::ProgressBar::new(self.progress).show_percentage());

            if !runtime_ready && local_profile {
                ui.small("Install/repair the runtime, authenticate Hugging Face, download the Harmonizer model pack, then refresh checks.");
            }
            if let Some(path) = &self.active_output {
                ui.monospace(format!("Active project: {}", path.display()));
            }
        });
    }

    fn log_ui(&mut self, ui: &mut egui::Ui) {
        ui.group(|ui| {
            ui.heading("Live log");
            egui::ScrollArea::vertical()
                .max_height(220.0)
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    for line in &self.logs {
                        ui.monospace(line);
                    }
                });
        });
    }

    fn start_reconstruction(&mut self) {
        let input = PathBuf::from(&self.input_video);
        let base_output = PathBuf::from(&self.output_dir);
        if !input.is_file() {
            self.notice = Some(format!("Input video does not exist: {}", input.display()));
            return;
        }

        let mut manifest = ProjectManifest::new(&input, &base_output, self.settings.clone());
        let project_dir = base_output.join(&manifest.project_id);
        manifest.output_dir = project_dir.display().to_string();
        if let Err(error) = manifest.save() {
            self.notice = Some(format!("Could not create project manifest: {error}"));
            return;
        }

        match self
            .runtime
            .pipeline_command(&input, &project_dir, &self.settings)
        {
            Ok((program, args)) => {
                self.active_output = Some(project_dir);
                self.progress = 0.0;
                self.logs.clear();
                self.start_task(program, args, "reconstruction");
            }
            Err(error) => self.notice = Some(error),
        }
    }

    fn start_task(&mut self, program: String, args: Vec<String>, label: &str) {
        match self.runner.start(program, args) {
            Ok(()) => {
                self.stage = label.to_owned();
                self.progress = 0.0;
                self.notice = None;
                self.push_log(format!("Started {label}"));
            }
            Err(error) => self.notice = Some(error),
        }
    }

    fn consume_events(&mut self) {
        for event in self.runner.poll() {
            match event {
                JobEvent::Log(line) => self.push_log(line),
                JobEvent::Stage { name, progress } => {
                    self.stage = name;
                    self.progress = progress;
                }
                JobEvent::Finished(code) => {
                    self.progress = 1.0;
                    self.stage = "completed".to_owned();
                    self.push_log(format!("Process completed successfully (code {code})"));
                    self.runtime_status = Some(self.runtime.inspect());
                }
                JobEvent::Failed(error) => {
                    self.stage = "failed".to_owned();
                    self.notice = Some(error.clone());
                    self.push_log(error);
                    self.runtime_status = Some(self.runtime.inspect());
                }
                JobEvent::Cancelled => {
                    self.stage = "cancelled".to_owned();
                    self.notice = Some("Task cancelled. Completed pipeline stages remain on disk and can be reused on the next run.".to_owned());
                    self.push_log("Task cancelled".to_owned());
                }
            }
        }
    }

    fn push_log(&mut self, line: String) {
        const MAX_LOG_LINES: usize = 1200;
        if self.logs.len() >= MAX_LOG_LINES {
            self.logs.pop_front();
        }
        self.logs.push_back(line);
    }
}

fn check_row(ui: &mut egui::Ui, label: &str, check: &Check) {
    ui.horizontal_wrapped(|ui| {
        ui.label(if check.ok { "OK" } else { "Missing" });
        ui.strong(label);
        ui.monospace(&check.detail);
    });
}

fn status_row(ui: &mut egui::Ui, label: &str, ok: bool) {
    ui.horizontal(|ui| {
        ui.label(if ok { "OK" } else { "Missing" });
        ui.strong(label);
    });
}

#[allow(dead_code)]
fn _is_windows_path(path: &Path) -> bool {
    path.is_absolute()
}
