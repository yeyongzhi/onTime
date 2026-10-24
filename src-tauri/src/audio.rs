use rodio::{DeviceSinkBuilder, Player, Source};
use std::{
    path::PathBuf,
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};
use tauri::{AppHandle, Emitter};

#[derive(Clone)]
pub struct AudioHandle(pub mpsc::Sender<Option<Play>>);
#[derive(Clone)]
pub struct Play {
    pub key: String,
    pub path: Option<PathBuf>,
    pub volume: f32,
    pub gradual: bool,
    pub tones: Option<Vec<f32>>,
}
impl AudioHandle {
    pub fn start(app: AppHandle) -> Self {
        let (tx, rx) = mpsc::channel::<Option<Play>>();
        thread::spawn(move || {
            let mut output = None;
            let mut player: Option<Player> = None;
            let mut active: Option<Play> = None;
            let mut started = Instant::now();
            let mut failed: Option<(String, Instant)> = None;
            let mut last_error = String::new();
            loop {
                match rx.recv_timeout(Duration::from_millis(100)) {
                    Ok(Some(play)) => {
                        if play.key == "#stop-alert" {
                            if !active
                                .as_ref()
                                .is_some_and(|p| p.key.starts_with("preview-"))
                            {
                                player.take();
                                output.take();
                                active = None;
                            }
                            continue;
                        }
                        if active.as_ref().map(|p| &p.key) == Some(&play.key) {
                            continue;
                        }
                        player.take();
                        output.take();
                        if failed.as_ref().is_some_and(|(key, at)| {
                            key == &play.key && at.elapsed() < Duration::from_secs(10)
                        }) {
                            continue;
                        }
                        match DeviceSinkBuilder::open_default_sink() {
                            Ok(mut device) => {
                                device.log_on_drop(false);
                                let p = Player::connect_new(device.mixer());
                                p.set_volume(if play.gradual {
                                    play.volume * 0.02
                                } else {
                                    play.volume
                                });
                                let mut custom = false;
                                if let Some(path) = &play.path {
                                    match std::fs::File::open(path)
                                        .ok()
                                        .and_then(|f| rodio::Decoder::try_from(f).ok())
                                    {
                                        Some(source) => {
                                            p.append(source.repeat_infinite());
                                            custom = true;
                                        }
                                        None => {
                                            let _ = app.emit(
                                                "audio-error",
                                                "自定义铃声不可用，已使用内置铃声",
                                            );
                                        }
                                    }
                                }
                                if let Some(tones) = &play.tones {
                                    let samples = tones.iter().flat_map(|frequency| {
                                        rodio::source::SineWave::new(*frequency).take_duration(Duration::from_millis(180)).amplify(0.18).collect::<Vec<f32>>().into_iter().chain(std::iter::repeat_n(0.0, 4800))
                                    }).collect::<Vec<f32>>();
                                    let source = rodio::buffer::SamplesBuffer::new(std::num::NonZeroU16::new(1).unwrap(), std::num::NonZeroU32::new(48000).unwrap(), samples);
                                    p.append(source.repeat_infinite());
                                    custom = true;
                                }
                                if !custom {
                                    // Soft two-note chime, silence between phrases.
                                    let chime = rodio::source::SineWave::new(660.0)
                                        .take_duration(Duration::from_millis(220))
                                        .amplify(0.16)
                                        .chain(
                                            rodio::source::SineWave::new(880.0)
                                                .take_duration(Duration::from_millis(320))
                                                .amplify(0.12),
                                        )
                                        .chain(
                                            rodio::source::SineWave::new(440.0)
                                                .take_duration(Duration::from_millis(950))
                                                .amplify(0.0),
                                        );
                                    p.append(
                                        rodio::buffer::SamplesBuffer::new(
                                            std::num::NonZeroU16::new(1).unwrap(),
                                            std::num::NonZeroU32::new(48000).unwrap(),
                                            chime.collect::<Vec<f32>>(),
                                        )
                                        .repeat_infinite(),
                                    );
                                }
                                started = Instant::now();
                                failed = None;
                                last_error.clear();
                                active = Some(play);
                                player = Some(p);
                                output = Some(device);
                            }
                            Err(e) => {
                                active = None;
                                failed = Some((play.key.clone(), Instant::now()));
                                let message = format!("音频设备不可用：{e}");
                                if last_error != message {
                                    let _ = app.emit("audio-error", &message);
                                    last_error = message;
                                }
                            }
                        }
                    }
                    Ok(None) => {
                        player.take();
                        output.take();
                        active = None;
                    }
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    Err(mpsc::RecvTimeoutError::Timeout) => {
                        let elapsed = started.elapsed().as_secs_f32();
                        if active
                            .as_ref()
                            .is_some_and(|p| p.key.starts_with("preview-") && elapsed >= 10.0)
                        {
                            player.take();
                            output.take();
                            active = None;
                        }
                        if let (Some(p), Some(play)) = (&player, &active)
                            && play.gradual {
                                p.set_volume(play.volume * (elapsed / 12.0).clamp(0.02, 1.0));
                            }
                    }
                }
            }
        });
        Self(tx)
    }
    pub fn play(&self, play: Play) {
        let _ = self.0.send(Some(play));
    }
    pub fn preview_tones(&self, frequencies: Vec<f32>, volume: f32) {
        self.play(Play { key: format!("preview-{}", uuid::Uuid::new_v4()), path: None, volume, gradual: false, tones: Some(frequencies) });
    }
    pub fn stop_alert(&self) {
        self.play(Play {
            key: "#stop-alert".into(),
            path: None,
            volume: 0.0,
            gradual: false,
            tones: None,
        });
    }
    pub fn stop(&self) {
        let _ = self.0.send(None);
    }
}
