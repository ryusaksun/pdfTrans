use std::time::Duration;

use crossterm::event::{self, Event as CrosstermEvent, KeyEvent, MouseEvent};
use tokio::sync::mpsc;

/// Application events combining terminal events and tick timer.
#[derive(Debug, Clone)]
pub enum AppEvent {
    Key(KeyEvent),
    Mouse(MouseEvent),
    Paste(String),
    Resize(u16, u16),
    Tick,
}

/// Reads terminal events in a background task and forwards them via channel.
pub struct EventReader {
    rx: mpsc::UnboundedReceiver<AppEvent>,
    _handle: tokio::task::JoinHandle<()>,
}

impl EventReader {
    /// Create a new event reader with the given tick rate.
    pub fn new(tick_rate: Duration) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        let handle = tokio::spawn(async move {
            loop {
                // Poll for crossterm events with tick_rate as timeout
                let has_event = tokio::task::spawn_blocking({
                    let tick_rate = tick_rate;
                    move || event::poll(tick_rate).unwrap_or(false)
                })
                .await
                .unwrap_or(false);

                if has_event {
                    let read_result =
                        tokio::task::spawn_blocking(|| event::read()).await;
                    if let Ok(Ok(evt)) = read_result {
                        let app_event = match evt {
                            CrosstermEvent::Key(key) => Some(AppEvent::Key(key)),
                            CrosstermEvent::Mouse(mouse) => Some(AppEvent::Mouse(mouse)),
                            CrosstermEvent::Paste(text) => Some(AppEvent::Paste(text)),
                            CrosstermEvent::Resize(w, h) => Some(AppEvent::Resize(w, h)),
                            _ => None,
                        };
                        if let Some(e) = app_event {
                            if tx.send(e).is_err() {
                                return;
                            }
                        }
                    }
                } else {
                    // Tick event when no input
                    if tx.send(AppEvent::Tick).is_err() {
                        return;
                    }
                }
            }
        });
        Self {
            rx,
            _handle: handle,
        }
    }

    /// Receive the next event.
    pub async fn next(&mut self) -> Option<AppEvent> {
        self.rx.recv().await
    }
}
