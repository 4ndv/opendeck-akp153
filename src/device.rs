use std::{process::exit, sync::Arc, thread::sleep, time::Duration};

use data_url::DataUrl;
use image::load_from_memory_with_format;
use mirajazz::{
    device::{Device, list_devices, new_hidapi},
    error::MirajazzError,
    state::{DeviceStateReader, DeviceStateUpdate},
};
use openaction::SetImageEvent;
use tokio::sync::mpsc::{self, Receiver, Sender};

use crate::{
    inputs::opendeck_to_device,
    mappings::{
        AJAZZ_VID, CandidateDevice, DEVICE_NAMESPACE, IMAGE_FORMAT, KEY_COUNT, Kind, MIRABOX_VID,
    },
};

const POLL_RATE_MS: u64 = 50;

#[derive(Debug)]
pub enum DeviceMessage {
    SetImage(String, SetImageEvent),
    SetBrightness(String, u8),
    Update(String, DeviceStateUpdate),
    Connected(String, Kind, Sender<DeviceMessage>),
    Disconnected(String),
}

/// Returns devices that matches known pid/vid pairs
pub fn get_candidates() -> Vec<CandidateDevice> {
    log::info!("Looking for candidate devices");

    let hidapi = match new_hidapi() {
        Ok(hidapi) => hidapi,
        Err(e) => {
            log::error!("Failed to init hidapi: {e}");
            exit(1);
        }
    };

    let mut candidates: Vec<CandidateDevice> = Vec::new();

    for (vid, pid, serial) in list_devices(&hidapi, &[AJAZZ_VID, MIRABOX_VID]) {
        let id = format!("{}-{}", DEVICE_NAMESPACE, serial);

        if let Some(kind) = Kind::from_vid_pid(vid, pid) {
            candidates.push(CandidateDevice {
                id,
                vid,
                pid,
                serial,
                kind,
            })
        } else {
            continue;
        }
    }

    candidates
}

/// Runs in a separate thread, handling events bound to a device
pub fn device_task(candidate: CandidateDevice, disp_tx: Sender<DeviceMessage>) {
    let (device_tx, mut device_rx) = mpsc::channel::<DeviceMessage>(1);

    let id = candidate.id.clone();

    log::info!("Connecting to {} from a device task", id);

    let device = match connect(&candidate) {
        Ok(device) => device,
        Err(err) => {
            return log::error!("Error while connecting to device {err}");
        }
    };

    disp_tx
        .blocking_send(DeviceMessage::Connected(
            candidate.id.clone(),
            candidate.kind.clone(),
            device_tx,
        ))
        .unwrap();

    let device = Arc::new(device);
    {
        let reader = device.get_reader();

        log::info!("Reader is ready for {}", id);

        loop {
            if let Err(err) = tick(id.clone(), &device, &mut device_rx, &disp_tx, &reader) {
                log::error!("Device {} error: {}", id, err);

                // Some errors are not critical and can be ignored without sending disconnected event
                if matches!(err, MirajazzError::ImageError(_) | MirajazzError::BadData) {
                    continue;
                }

                disp_tx
                    .blocking_send(DeviceMessage::Disconnected(id.clone()))
                    .unwrap();

                break;
            }

            sleep(Duration::from_millis(POLL_RATE_MS));
        }

        drop(reader);
    }
}

fn connect(candidate: &CandidateDevice) -> Result<Device, MirajazzError> {
    let hidapi = match new_hidapi() {
        Ok(hidapi) => hidapi,
        Err(e) => {
            log::error!("Failed to init hidapi: {e}");
            exit(1);
        }
    };

    let device = Device::connect(
        &hidapi,
        candidate.vid,
        candidate.pid,
        &candidate.serial,
        false,
        candidate.kind.supports_both_states(),
        KEY_COUNT,
        0,
    )?;

    log::info!("Connected to {}", candidate.id);

    log::info!("Resetting a device");

    device.set_brightness(50)?;
    device.clear_all_button_images()?;
    device.flush()?;

    Ok(device)
}

fn tick(
    id: String,
    device: &Device,
    device_rx: &mut Receiver<DeviceMessage>,
    disp_tx: &Sender<DeviceMessage>,
    reader: &Arc<DeviceStateReader>,
) -> Result<(), MirajazzError> {
    if let Ok(message) = device_rx.try_recv() {
        log::debug!("Device task got message: {:#?}", message);

        match message {
            DeviceMessage::SetImage(_, evt) => {
                handle_set_image(&device, evt)?;
            }
            DeviceMessage::SetBrightness(_, brightness) => {
                device.set_brightness(brightness)?;
            }
            _ => {}
        }
    }

    let updates = reader.read(None, crate::inputs::process_input)?;

    for update in updates {
        log::debug!("New update: {:#?}", update);

        disp_tx
            .blocking_send(DeviceMessage::Update(id.clone(), update))
            .unwrap();
    }

    Ok(())
}

/// Handles different combinations of "set image" event, including clearing the specific buttons and whole device
fn handle_set_image(device: &Device, evt: SetImageEvent) -> Result<(), MirajazzError> {
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

            device.set_button_image(opendeck_to_device(position), IMAGE_FORMAT, image)?;
            device.flush()?;
        }
        (Some(position), None) => {
            device.clear_button_image(opendeck_to_device(position))?;
            device.flush()?;
        }
        (None, None) => {
            device.clear_all_button_images()?;
            device.flush()?;
        }
        _ => {}
    }

    Ok(())
}
