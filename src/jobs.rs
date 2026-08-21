use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobStage {
    Preparing,
    ExtractingFrames,
    SolvingCamera,
    TrainingSplat,
    Harmonizing,
    Refining,
    Exporting,
    Completed,
    Failed,
}

#[derive(Debug, Clone)]
pub struct ReconstructionJob {
    pub input_video: PathBuf,
    pub output_dir: PathBuf,
    pub stage: JobStage,
    pub progress: f32,
}

impl ReconstructionJob {
    pub fn new(input_video: PathBuf, output_dir: PathBuf) -> Self {
        Self {
            input_video,
            output_dir,
            stage: JobStage::Preparing,
            progress: 0.0,
        }
    }

    pub fn next_stage(&mut self) {
        self.stage = match self.stage {
            JobStage::Preparing => JobStage::ExtractingFrames,
            JobStage::ExtractingFrames => JobStage::SolvingCamera,
            JobStage::SolvingCamera => JobStage::TrainingSplat,
            JobStage::TrainingSplat => JobStage::Harmonizing,
            JobStage::Harmonizing => JobStage::Refining,
            JobStage::Refining => JobStage::Exporting,
            JobStage::Exporting => JobStage::Completed,
            JobStage::Completed | JobStage::Failed => return,
        };
    }
}
