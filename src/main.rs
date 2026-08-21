mod models;
mod pipeline;
mod system;

use eframe::egui;
use models::ModelPack;
use pipeline::RuntimeProfile;
use system::{Check, SystemReport};

fn main() -> eframe::Result {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([980.0, 760.0])
            .with_min_inner_size([760.0, 580.0]),
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
    report: Option<SystemReport>,
}

impl Default for StudioApp {
    fn default() -> Self {
        Self {
            profile: RuntimeProfile::Local16Gb,
            model_pack: ModelPack::LocalRecommended,
            input_video: String::new(),
            output_dir: String::new(),
            report: None,
        }
    }
}

impl eframe::App for StudioApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        ui.heading("Gaussian Splat Studio");
        ui.label("Video -> 3DGS -> NVIDIA Harmonizer -> refinement -> PLY/SPZ");
        ui.add_space(10.0);

        ui.group(|ui| {
            ui.heading("1. System");
            ui.label(
                "The Windows app orchestrates CUDA workloads through local tools or WSL2/Docker.",
            );
            if ui.button("Run system check").clicked() {
                self.report = Some(system::inspect());
            }

            if let Some(report) = &self.report {
                check_row(ui, "NVIDIA GPU", &report.gpu);
                check_row(ui, "WSL2", &report.wsl);
                check_row(ui, "Docker", &report.docker);
            }
        });

        ui.add_space(10.0);
        ui.group(|ui| {
            ui.heading("2. Runtime profile");
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

            match self.profile {
                RuntimeProfile::Local16Gb => {
                    ui.label("Runs reconstruction and Harmonizer as separate GPU stages to keep peak VRAM down.");
                }
                RuntimeProfile::FullNuRec => {
                    ui.label("Requires a Linux NuRec host with more than 24 GB VRAM; 48 GB+ is preferred by NVIDIA.");
                }
            }
        });

        ui.add_space(10.0);
        ui.group(|ui| {
            ui.heading("3. Model/runtime pack");
            egui::ComboBox::from_id_salt("model_pack")
                .selected_text(self.model_pack.info().title)
                .show_ui(ui, |ui| {
                    for pack in ModelPack::ALL {
                        ui.selectable_value(&mut self.model_pack, pack, pack.info().title);
                    }
                });

            let info = self.model_pack.info();
            ui.label(format!(
                "Approx. payload: {:.2} GB; recommended free disk: {} GB",
                info.model_download_gb, info.free_disk_gb
            ));
            ui.small(info.description);
        });

        ui.add_space(10.0);
        ui.group(|ui| {
            ui.heading("4. Capture");
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
                if ui.button("Choose output folder").clicked() {
                    if let Some(path) = rfd::FileDialog::new().pick_folder() {
                        self.output_dir = path.display().to_string();
                    }
                }
                ui.monospace(if self.output_dir.is_empty() {
                    "No output folder selected"
                } else {
                    &self.output_dir
                });
            });
        });

        ui.add_space(10.0);
        ui.group(|ui| {
            ui.heading("5. Planned pipeline");
            for (index, step) in pipeline::plan(self.profile).iter().enumerate() {
                ui.label(format!("{}. {}", index + 1, step.name));
                ui.small(step.detail);
                ui.add_space(4.0);
            }
        });

        ui.add_space(10.0);
        ui.horizontal(|ui| {
            let ready = !self.input_video.is_empty() && !self.output_dir.is_empty();
            ui.add_enabled(ready, egui::Button::new("Start reconstruction"));
            ui.label("Execution wiring is the next implementation milestone.");
        });
    }
}

fn check_row(ui: &mut egui::Ui, label: &str, check: &Check) {
    ui.horizontal_wrapped(|ui| {
        ui.label(if check.ok { "OK" } else { "Missing" });
        ui.strong(label);
        ui.monospace(&check.detail);
    });
}
