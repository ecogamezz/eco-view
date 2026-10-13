use std::process::{Child, Command};
use std::sync::mpsc::{self, Sender};
use std::thread;
use std::time::Duration;

#[derive(Clone, Copy, Debug)]
pub enum Cue {
    Click,
    Accent,
    Warn,
    // The raccoon's little noises.
    Pickup,
    Whoosh,
    Thud,
    Boop,
    Purr,
    Cheer,
    Alarm,
}

impl Cue {
    /// System sound and a loudness relative to the user's volume.
    fn file(self) -> (&'static str, f32) {
        match self {
            Cue::Click => ("Tink", 1.0),
            Cue::Accent => ("Pop", 1.0),
            Cue::Warn => ("Sosumi", 1.0),
            Cue::Pickup => ("Tink", 0.7),
            Cue::Whoosh => ("Blow", 0.5),
            Cue::Thud => ("Bottle", 0.45),
            Cue::Boop => ("Pop", 0.8),
            Cue::Purr => ("Purr", 0.8),
            Cue::Cheer => ("Hero", 0.5),
            Cue::Alarm => ("Funk", 0.6),
        }
    }
}

#[derive(Clone)]
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
            // Players run concurrently so a burst of cues never queues up
            // behind one another; finished ones are reaped as we go.
            let mut playing: Vec<Child> = Vec::new();
            loop {
                let cue = match rx.recv_timeout(Duration::from_secs(1)) {
                    Ok(c) => Some(c),
                    Err(mpsc::RecvTimeoutError::Timeout) => None,
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                };
                playing.retain_mut(|c| matches!(c.try_wait(), Ok(None)));
                let Some(cue) = cue else { continue };
                let (enabled, vol) = volume.lock().map(|g| *g).unwrap_or((false, 0.0));
                if !enabled || vol <= 0.01 || playing.len() >= 4 {
                    continue;
                }
                let (name, gain) = cue.file();
                let file = format!("/System/Library/Sounds/{name}.aiff");
                if let Ok(child) = Command::new("afplay")
                    .args(["-v", &format!("{:.2}", vol * gain), &file])
                    .spawn()
                {
                    playing.push(child);
                }
            }
        })
        .ok();
    SoundTx(tx)
}
