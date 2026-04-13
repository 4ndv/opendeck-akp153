use device::{handle_set_image, handle_error};
use mirajazz::device::Device;
use openaction::*;
use std::{collections::HashMap, sync::LazyLock, time::Duration};
use tokio::sync::{Mutex, RwLock};
use tokio_util::{sync::CancellationToken, task::TaskTracker};

#[cfg(not(target_os = "windows"))]
use tokio::signal::unix::{signal, SignalKind};

mod device;
mod inputs;
mod mappings;
mod watcher;

pub static DEVICES: LazyLock<RwLock<HashMap<String, Device>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));
pub static TOKENS: LazyLock<RwLock<HashMap<String, CancellationToken>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));
pub static TRACKER: LazyLock<Mutex<TaskTracker>> =
    LazyLock::new(|| Mutex::new(TaskTracker::new()));

struct GlobalEventHandler {}

impl openaction::GlobalEventHandler for GlobalEventHandler {
    async fn plugin_ready(
        &self,
        _outbound: &mut openaction::OutboundEventManager,
    ) -> EventHandlerResult {
        let tracker = TRACKER.lock().await.clone();
        let token = CancellationToken::new();

        tracker.spawn(watcher::watcher_task(token.clone()));
        TOKENS
            .write()
            .await
            .insert("_watcher_task".to_string(), token);

        log::info!("Plugin initialized");
        Ok(())
    }

    async fn set_image(
        &self,
        event: SetImageEvent,
        _outbound: &mut OutboundEventManager,
    ) -> EventHandlerResult {
        log::debug!("Asked to set image: {:#?}", event);

        if event.controller == Some("Encoder".to_string()) {
            log::debug!("Looks like a knob, no need to set image");
            return Ok(());
        }

        let id = event.device.clone();

        if let Some(device) = DEVICES.read().await.get(&event.device) {
            let _ = handle_set_image(device, event)
                .await
                .map_err(|err| async {
                    handle_error(&id, err, false).await;
                });
        } else {
            log::error!("Received event for unknown device: {}", event.device);
        }

        Ok(())
    }

    async fn set_brightness(
        &self,
        event: SetBrightnessEvent,
        _outbound: &mut OutboundEventManager,
    ) -> EventHandlerResult {
        log::debug!("Asked to set brightness: {:#?}", event);

        let id = event.device.clone();

        if let Some(device) = DEVICES.read().await.get(&event.device) {
            let _ = device
                .set_brightness(event.brightness)
                .await
                .map_err(|err| async {
                    handle_error(&id, err, false).await;
                });
        } else {
            log::error!("Received event for unknown device: {}", event.device);
        }

        Ok(())
    }
}

struct ActionEventHandler {}
impl openaction::ActionEventHandler for ActionEventHandler {}

async fn shutdown() {
    let tokens = TOKENS.write().await;
    for (_, token) in tokens.iter() {
        token.cancel();
    }
}

async fn cleanup_runtime() {
    shutdown().await;

    let tracker = TRACKER.lock().await.clone();
    tracker.wait().await;

    TOKENS.write().await.clear();
    DEVICES.write().await.clear();
}

async fn connect_loop(stop: CancellationToken) {
    while !stop.is_cancelled() {
        match init_plugin(GlobalEventHandler {}, ActionEventHandler {}).await {
            Ok(_) => {
                log::warn!("Disconnected from OpenDeck, retrying in 2 seconds");
            }
            Err(error) => {
                log::error!("Failed to initialize plugin: {}", error);
            }
        }

        cleanup_runtime().await;

        tokio::select! {
            _ = stop.cancelled() => break,
            _ = tokio::time::sleep(Duration::from_secs(2)) => {}
        }
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
async fn sigterm() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut sig = signal(SignalKind::terminate())?;
    sig.recv().await;
    Ok(())
}

#[cfg(target_os = "windows")]
async fn sigterm() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    std::future::pending::<()>().await;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    simplelog::TermLogger::init(
        simplelog::LevelFilter::Info,
        simplelog::Config::default(),
        simplelog::TerminalMode::Stdout,
        simplelog::ColorChoice::Never,
    )
    .unwrap();

    let stop = CancellationToken::new();
    let stop_for_loop = stop.clone();

    tokio::select! {
        _ = connect_loop(stop_for_loop) => {},
        _ = sigterm() => {
            log::info!("Received SIGTERM");
            stop.cancel();
        }
    }

    log::info!("Shutting down");
    cleanup_runtime().await;

    let tracker = TRACKER.lock().await.clone();
    tracker.close();
    tracker.wait().await;

    log::info!("Tasks are finished, exiting now");
    Ok(())
}
