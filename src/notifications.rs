use std::process::Command;
use std::sync::mpsc::{self, Sender};
use std::thread;

pub struct NotifyTx(Sender<String>);

impl NotifyTx {
    pub fn post(&self, body: impl Into<String>) {
        let _ = self.0.send(body.into());
    }
}

pub fn spawn() -> NotifyTx {
    let (tx, rx) = mpsc::channel::<String>();
    thread::Builder::new()
        .name("eco-notify".into())
        .spawn(move || {
            while let Ok(body) = rx.recv() {
                let escaped = body.replace('\\', "\\\\").replace('"', "\\\"");
                let script = format!(
                    r#"display notification "{escaped}" with title "eco view""#
                );
                let _ = Command::new("osascript").args(["-e", &script]).status();
            }
        })
        .ok();
    NotifyTx(tx)
}
