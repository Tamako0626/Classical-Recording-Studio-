//! 试听. 本机能直接解码的文件从指定位置播放, 其余格式先由 ffmpeg 写成一段 WAV.

use std::path::PathBuf;
use std::sync::mpsc::{self, Sender};
use std::time::Duration;

use crate::error::{StudioError, StudioResult};

enum PlayCmd {
    Play { path: PathBuf, offset: Duration, ack: Sender<Result<(), String>> },
    Stop,
}

pub struct Player {
    tx: Sender<PlayCmd>,
}

impl Player {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel::<PlayCmd>();
        std::thread::spawn(move || {
            let mut stream = None;
            let mut sink: Option<rodio::Sink> = None;
            while let Ok(cmd) = rx.recv() {
                match cmd {
                    PlayCmd::Stop => {
                        if let Some(sink) = sink.take() {
                            sink.stop();
                        }
                    }
                    PlayCmd::Play { path, offset, ack } => {
                        if let Some(sink) = sink.take() {
                            sink.stop();
                        }
                        let opened = (|| -> Result<(), String> {
                            if stream.is_none() {
                                stream = Some(rodio::OutputStream::try_default().map_err(|error| error.to_string())?);
                            }
                            let handle = &stream.as_ref().unwrap().1;
                            let next = rodio::Sink::try_new(handle).map_err(|error| error.to_string())?;
                            let file = std::fs::File::open(&path).map_err(|error| error.to_string())?;
                            let source = rodio::Decoder::new(std::io::BufReader::new(file)).map_err(|error| error.to_string())?;
                            next.append(source);
                            if offset > Duration::from_millis(80) {
                                if let Err(error) = next.try_seek(offset) {
                                    next.stop();
                                    return Err(error.to_string());
                                }
                            }
                            sink = Some(next);
                            Ok(())
                        })();
                        let _ = ack.send(opened);
                    }
                }
            }
        });
        Self { tx }
    }

    pub fn play(&self, path: PathBuf) -> StudioResult<()> {
        self.play_at(path, Duration::from_secs(0))
    }

    pub fn play_at(&self, path: PathBuf, offset: Duration) -> StudioResult<()> {
        let (ack_tx, ack_rx) = mpsc::channel();
        self.tx
            .send(PlayCmd::Play { path, offset, ack: ack_tx })
            .map_err(|_| StudioError::new("播放器已经退出."))?;
        match ack_rx.recv_timeout(Duration::from_secs(8)) {
            Ok(Ok(())) => Ok(()),
            Ok(Err(message)) => Err(StudioError::new(message)),
            Err(_) => Err(StudioError::new("播放器没有及时响应.")),
        }
    }

    pub fn stop(&self) {
        let _ = self.tx.send(PlayCmd::Stop);
    }
}

impl Default for Player {
    fn default() -> Self {
        Self::new()
    }
}
