use serde_json::Value;
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

#[derive(Debug, Clone)]
pub enum JobEvent {
    Log(String),
    Stage { name: String, progress: f32 },
    Finished(i32),
    Failed(String),
    Cancelled,
}

pub struct JobRunner {
    receiver: Receiver<JobEvent>,
    sender: Sender<JobEvent>,
    cancel: Arc<AtomicBool>,
    running: Arc<AtomicBool>,
}

impl Default for JobRunner {
    fn default() -> Self {
        let (sender, receiver) = mpsc::channel();
        Self {
            receiver,
            sender,
            cancel: Arc::new(AtomicBool::new(false)),
            running: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl JobRunner {
    pub fn start(&self, program: String, args: Vec<String>) -> Result<(), String> {
        if self.running.swap(true, Ordering::SeqCst) {
            return Err("A reconstruction job is already running".to_owned());
        }

        self.cancel.store(false, Ordering::SeqCst);
        let sender = self.sender.clone();
        let cancel = Arc::clone(&self.cancel);
        let running = Arc::clone(&self.running);

        thread::spawn(move || {
            let result = run_process(program, args, &sender, &cancel);
            if let Err(error) = result {
                let _ = sender.send(JobEvent::Failed(error));
            }
            running.store(false, Ordering::SeqCst);
        });

        Ok(())
    }

    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::SeqCst);
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    pub fn poll(&self) -> Vec<JobEvent> {
        let mut events = Vec::new();
        while let Ok(event) = self.receiver.try_recv() {
            events.push(event);
        }
        events
    }
}

fn run_process(
    program: String,
    args: Vec<String>,
    event_tx: &Sender<JobEvent>,
    cancel: &AtomicBool,
) -> Result<(), String> {
    let mut child = Command::new(&program)
        .args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("Failed to start {program}: {error}"))?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "Failed to capture pipeline stdout".to_owned())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "Failed to capture pipeline stderr".to_owned())?;

    let (line_tx, line_rx) = mpsc::channel::<String>();
    spawn_line_reader(stdout, line_tx.clone());
    spawn_line_reader(stderr, line_tx);

    loop {
        while let Ok(line) = line_rx.try_recv() {
            emit_line(event_tx, line);
        }

        if cancel.load(Ordering::SeqCst) {
            let _ = child.kill();
            let _ = child.wait();
            let _ = event_tx.send(JobEvent::Cancelled);
            return Ok(());
        }

        match child.try_wait() {
            Ok(Some(status)) => {
                while let Ok(line) = line_rx.recv_timeout(Duration::from_millis(10)) {
                    emit_line(event_tx, line);
                }
                let code = status.code().unwrap_or(-1);
                if status.success() {
                    let _ = event_tx.send(JobEvent::Finished(code));
                    return Ok(());
                }
                return Err(format!("Pipeline exited with code {code}"));
            }
            Ok(None) => thread::sleep(Duration::from_millis(50)),
            Err(error) => return Err(format!("Failed while waiting for pipeline: {error}")),
        }
    }
}

fn spawn_line_reader<R>(reader: R, sender: Sender<String>)
where
    R: std::io::Read + Send + 'static,
{
    thread::spawn(move || {
        for line in BufReader::new(reader).lines().map_while(Result::ok) {
            let _ = sender.send(line);
        }
    });
}

fn emit_line(sender: &Sender<JobEvent>, line: String) {
    if let Some(payload) = line.strip_prefix("GSS_EVENT ") {
        if let Ok(value) = serde_json::from_str::<Value>(payload) {
            let name = value
                .get("stage")
                .and_then(Value::as_str)
                .unwrap_or("working")
                .to_owned();
            let progress = value
                .get("progress")
                .and_then(Value::as_f64)
                .unwrap_or(0.0)
                .clamp(0.0, 1.0) as f32;
            let _ = sender.send(JobEvent::Stage { name, progress });
            if let Some(message) = value.get("message").and_then(Value::as_str) {
                let _ = sender.send(JobEvent::Log(message.to_owned()));
            }
            return;
        }
    }

    let _ = sender.send(JobEvent::Log(line));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn structured_event_is_parsed() {
        let (tx, rx) = mpsc::channel();
        emit_line(
            &tx,
            "GSS_EVENT {\"stage\":\"frames\",\"progress\":0.25}".to_owned(),
        );
        match rx.recv().unwrap() {
            JobEvent::Stage { name, progress } => {
                assert_eq!(name, "frames");
                assert!((progress - 0.25).abs() < f32::EPSILON);
            }
            other => panic!("unexpected event: {other:?}"),
        }
    }
}
