use std::time::Instant;
use futures_lite::StreamExt;
use mirajazz::{
    device::{DeviceWatcher, list_devices},
    error::MirajazzError,
    types::{DeviceLifecycleEvent, HidDeviceInfo},
};
use openaction::OUTBOUND_EVENT_MANAGER;
use tokio_util::sync::CancellationToken;

use crate::{
    DEVICES, TOKENS, TRACKER,
    device::device_task,
    mappings::{CandidateDevice, DEVICE_NAMESPACE, Kind, QUERIES},
};

const SLEEP_THRESHOLD: std::time::Duration = std::time::Duration::from_secs(3);

fn get_device_id(dev: &HidDeviceInfo) -> Option<String> {
    let kind = Kind::from_vid_pid(dev.vendor_id, dev.product_id)?;

    match kind.protocol_version() {
        2 | 3 => Some(format!(
            "{}-{}",
            DEVICE_NAMESPACE,
            dev.serial_number.clone()?,
        )),
        1 => {
            // All the "v1" devices share the same serial. Hardcode it because Windows returns invalid serial for them
            // Also suffix v1 devices with the
            Some(format!(
                "{}-355499441494-{}",
                DEVICE_NAMESPACE,
                kind.id_suffix()
            ))
        }
        _ => unreachable!(),
    }
}

fn device_info_to_candidate(dev: HidDeviceInfo) -> Option<CandidateDevice> {
    let id = get_device_id(&dev)?;
    let kind = Kind::from_vid_pid(dev.vendor_id, dev.product_id)?;

    Some(CandidateDevice { id, dev, kind })
}

/// Returns devices that matches known pid/vid pairs
async fn get_candidates() -> Result<Vec<CandidateDevice>, MirajazzError> {
    log::info!("Looking for candidate devices");

    let mut candidates: Vec<CandidateDevice> = Vec::new();

    for dev in list_devices(&QUERIES).await? {
        if let Some(candidate) = device_info_to_candidate(dev.clone()) {
            candidates.push(candidate);
        } else {
            continue;
        }
    }

    Ok(candidates)
}

pub async fn watcher_task(token: CancellationToken) -> Result<(), MirajazzError> {
    let tracker = TRACKER.lock().await.clone();

    'outer: loop {
        if token.is_cancelled() {
            break 'outer Ok(());
        }

        // Cancel all device tasks from previous iteration.
        {
            let mut tokens = TOKENS.write().await;
            tokens.retain(|id, tok| {
                if id != "_watcher_task" {
                    tok.cancel();
                }
                id == "_watcher_task"
            });
        }
        DEVICES.write().await.clear();

        // Give old device_tasks time to finish cleanup before rescanning.
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        let candidates = get_candidates().await?;

        log::info!("Looking for connected devices");

        for candidate in candidates {
            log::info!("New candidate {:#?}", candidate);

            let token = CancellationToken::new();

            TOKENS
                .write()
                .await
                .insert(candidate.id.clone(), token.clone());

            tracker.spawn(device_task(candidate, token));
        }

        let mut watcher = DeviceWatcher::new();
        let mut watcher_stream = match watcher.watch(&QUERIES).await {
            Ok(stream) => stream,
            Err(err) => {
                log::error!("Failed to start watcher: {err}, retrying in 3 seconds");
                tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                continue;
            }
        };

        log::info!("Watcher is ready");

        let mut last_check = Instant::now();

        loop {
            let ev = tokio::select! {
                v = watcher_stream.next() => v,
                _ = token.cancelled() => None,
                _ = tokio::time::sleep(std::time::Duration::from_secs(1)) => {
                    let elapsed = last_check.elapsed();
                    last_check = Instant::now();

                    if elapsed > SLEEP_THRESHOLD {
                        log::warn!(
                            "System sleep detected ({:?} gap), restarting device discovery",
                            elapsed
                        );
                        last_check = Instant::now();
                        continue 'outer;
                    }

                    continue;
                }
            };

            if let Some(ev) = ev {
                log::info!("New device event: {:?}", ev);

                match ev {
                    DeviceLifecycleEvent::Connected(info) => {
                        if let Some(candidate) = device_info_to_candidate(info) {
                            if DEVICES.read().await.contains_key(&candidate.id) {
                                continue;
                            }

                            let token = CancellationToken::new();

                            TOKENS
                                .write()
                                .await
                                .insert(candidate.id.clone(), token.clone());

                            log::debug!("Spawning task for new device: {:?}", candidate);
                            tracker.spawn(device_task(candidate, token));
                            log::debug!("Spawned");
                        }
                    }
                    DeviceLifecycleEvent::Disconnected(info) => {
                        let id = get_device_id(&info)
                            .expect("Unable to get device id, check mappings in Kind::from_vid_pid");

                        if let Some(token) = TOKENS.write().await.remove(&id) {
                            log::info!("Sending cancel request for {}", id);
                            token.cancel();
                        }

                        DEVICES.write().await.remove(&id);

                        if let Some(outbound) = OUTBOUND_EVENT_MANAGER.lock().await.as_mut() {
                            outbound.deregister_device(id.clone()).await.ok();
                        }

                        log::info!("Disconnected device {}", id);
                    }
                }
            } else {
                log::info!("Watcher stream ended");

                if token.is_cancelled() {
                    break 'outer Ok(());
                }

                log::warn!("Watcher ended unexpectedly, restarting in 3 seconds");
                tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                break;
            }
        }
    }
}
