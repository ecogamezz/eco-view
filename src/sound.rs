use std::process::Command;
use std::sync::mpsc::{self, Sender};
use std::thread;

#[derive(Clone, Copy)]
pub enum Cue {
    Click,
    Accent,
    Warn,
}

pub struct SoundTx(Sender<Cue>);

impl SoundTx {
    pub fn play(&self, cue: Cue) {
        let _ = self.0.send(cue);
    }
}

pub fn spawn(volume: std::sync::Arc<std::sync::Mutex<(bool, f32)>>) -> SoundTx {
    let (tx, rx) = mpsc::channel::<Cue>();
    thread::Builder::new()
        .name("eco-sound".into())
        .spawn(move || {
            while let Ok(cue) = rx.recv() {
                let (enabled, vol) = volume.lock().map(|g| *g).unwrap_or((false, 0.0));
                if !enabled || vol <= 0.01 {
                    continue;
                }
                let file = match cue {
                    Cue::Click => "/System/Library/Sounds/Tink.aiff",
                    Cue::Accent => "/System/Library/Sounds/Pop.aiff",
                    Cue::Warn => "/System/Library/Sounds/Sosumi.aiff",
                };
                let _ = Command::new("afplay")
                    .args(["-v", &format!("{vol:.2}"), file])
                    .status();
            }
        })
        .ok();
    SoundTx(tx)
}
