use data_url::DataUrl;
use image::load_from_memory_with_format;
use mirajazz::{device::Device, error::MirajazzError, state::DeviceStateUpdate};
use openaction::{OUTBOUND_EVENT_MANAGER, SetImageEvent};
use tokio_util::sync::CancellationToken;
use std::time::Duration;

use crate::{
    DEVICES, TOKENS,
    inputs::opendeck_to_device,
    mappings::{
        get_image_format_for_key, CandidateDevice, Kind, COL_COUNT, ENCODER_COUNT, KEY_COUNT,
        ROW_COUNT,
    },
};

pub async fn device_task(candidate: CandidateDevice, token: CancellationToken) {
    log::info!("Running device supervisor for {:?}", candidate);

    let mut retry_delay_secs = 2u64;

    loop {
        if token.is_cancelled() {
            log::info!("Device task cancelled for {:?}", candidate);
            break;
        }

        let result = run_device_session(&candidate, token.clone()).await;

        match result {
            Ok(()) => {
                if token.is_cancelled() {
                    break;
                }

                log::warn!(
                    "Device session for {} ended unexpectedly, retrying in {}s",
                    candidate.id,
                    retry_delay_secs
                );
            }
            Err(err) => {
                handle_error(&candidate.id, err, false).await;
                log::warn!(
                    "Device session for {} crashed, retrying in {}s",
                    candidate.id,
                    retry_delay_secs
                );
            }
        }

        tokio::select! {
            _ = token.cancelled() => break,
            _ = tokio::time::sleep(Duration::from_secs(retry_delay_secs)) => {}
        }

        retry_delay_secs = (retry_delay_secs * 2).min(30);
    }

    let _ = DEVICES.write().await.remove(&candidate.id);
    log::info!("Device task finished for {:?}", candidate);
}

async fn run_device_session(
    candidate: &CandidateDevice,
    token: CancellationToken,
) -> Result<(), MirajazzError> {
    let device = connect(candidate).await?;
    device.set_brightness(50).await?;
    device.clear_all_button_images().await?;
    device.flush().await?;

    log::info!("Registering device {}", candidate.id);
    if let Some(outbound) = OUTBOUND_EVENT_MANAGER.lock().await.as_mut() {
        outbound
            .register_device(
                candidate.id.clone(),
                candidate.kind.human_name(),
                ROW_COUNT as u8,
                COL_COUNT as u8,
                ENCODER_COUNT as u8,
                0,
            )
            .await
            .unwrap();
    }

    DEVICES
        .write()
        .await
        .insert(candidate.id.clone(), device);

    let result = tokio::select! {
        res = device_events_task(candidate) => res,
        _ = token.cancelled() => Ok(()),
    };

    if let Some(device) = DEVICES.read().await.get(&candidate.id) {
        let _ = device.shutdown().await;
    }

    DEVICES.write().await.remove(&candidate.id);

    result
}

pub async fn handle_error(id: &str, err: MirajazzError, cancel_task: bool) -> bool {
    log::error!("Device {} error: {}", id, err);

    if matches!(err, MirajazzError::ImageError(_) | MirajazzError::BadData) {
        return true;
    }

    log::info!("Deregistering device {}", id);
    if let Some(outbound) = OUTBOUND_EVENT_MANAGER.lock().await.as_mut() {
        let _ = outbound.deregister_device(id.to_string()).await;
    }

    if cancel_task {
        log::info!("Cancelling tasks for device {}", id);
        if let Some(token) = TOKENS.read().await.get(id) {
            token.cancel();
        }
    }

    log::info!("Removing device {} from the list", id);
    DEVICES.write().await.remove(id);
    log::info!("Finished clean-up for {}", id);

    false
}

pub async fn connect(candidate: &CandidateDevice) -> Result<Device, MirajazzError> {
    let result = Device::connect(
        &candidate.dev,
        candidate.kind.protocol_version(),
        KEY_COUNT,
        ENCODER_COUNT,
    )
    .await;

    match result {
        Ok(device) => Ok(device),
        Err(e) => {
            log::error!("Error while connecting to device: {e}");
            Err(e)
        }
    }
}

/// Handles events from device to OpenDeck
async fn device_events_task(candidate: &CandidateDevice) -> Result<(), MirajazzError> {
    log::info!("Connecting to {} for incoming events", candidate.id);

    let devices_lock = DEVICES.read().await;
    let reader = match devices_lock.get(&candidate.id) {
        Some(device) => device.get_reader(crate::inputs::process_input),
        None => return Ok(()),
    };
    drop(devices_lock);

    log::info!("Connected to {} for incoming events", candidate.id);

    log::info!("Reader is ready for {}", candidate.id);

    loop {
        log::info!("Reading updates...");

        let updates = match reader.read(None).await {
            Ok(updates) => updates,
            Err(e) => {
                if !handle_error(&candidate.id, e, false).await {
                    break;
                }

                continue;
            }
        };

        for update in updates {
            log::info!("New update: {:#?}", update);

            let id = candidate.id.clone();

            if let Some(outbound) = OUTBOUND_EVENT_MANAGER.lock().await.as_mut() {
                match update {
                    DeviceStateUpdate::ButtonDown(key) => outbound.key_down(id, key).await.unwrap(),
                    DeviceStateUpdate::ButtonUp(key) => outbound.key_up(id, key).await.unwrap(),
                    DeviceStateUpdate::EncoderDown(encoder) => {
                        outbound.encoder_down(id, encoder).await.unwrap();
                    }
                    DeviceStateUpdate::EncoderUp(encoder) => {
                        outbound.encoder_up(id, encoder).await.unwrap();
                    }
                    DeviceStateUpdate::EncoderTwist(encoder, val) => {
                        outbound
                            .encoder_change(id, encoder, val as i16)
                            .await
                            .unwrap();
                    }
                }
            }
        }
    }

    Ok(())
}

/// Handles different combinations of "set image" event, including clearing the specific buttons and whole device
pub async fn handle_set_image(device: &Device, evt: SetImageEvent) -> Result<(), MirajazzError> {
    match (evt.position, evt.image) {
        (Some(position), Some(image)) => {
            log::info!("Setting image for button {}", position);

            // OpenDeck sends image as a data url, so parse it using a library
            let url = DataUrl::process(image.as_str()).unwrap(); // Isn't expected to fail, so unwrap it is
            let (body, _fragment) = url.decode_to_vec().unwrap(); // Same here

            // Allow only image/jpeg mime for now
            if url.mime_type().subtype != "jpeg" {
                log::error!("Incorrect mime type: {}", url.mime_type());

                return Ok(()); // Not a fatal error, enough to just log it
            }

            let image = load_from_memory_with_format(body.as_slice(), image::ImageFormat::Jpeg)?;

            let kind = Kind::from_vid_pid(device.vid, device.pid).unwrap(); // Safe to unwrap here, because device is already filtered

            device
                .set_button_image(
                    opendeck_to_device(position),
                    get_image_format_for_key(&kind, position),
                    image,
                )
                .await?;
            device.flush().await?;
        }
        (Some(position), None) => {
            device
                .clear_button_image(opendeck_to_device(position))
                .await?;
            device.flush().await?;
        }
        (None, None) => {
            device.clear_all_button_images().await?;
            device.flush().await?;
        }
        _ => {}
    }

    Ok(())
}
