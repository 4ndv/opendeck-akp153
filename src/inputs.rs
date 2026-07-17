use mirajazz::{error::MirajazzError, state::DeviceStateUpdate, types::DeviceInput};

use crate::mappings::KEY_COUNT;

pub fn process_input(input: u8, state: u8) -> Result<DeviceInput, MirajazzError> {
    log::info!("Processing input: {}, {}", input, state);

    match input as usize {
        (0..=KEY_COUNT) => read_button_press(input, state),
        _ => Err(MirajazzError::BadData),
    }
}

fn read_button_states(states: &[u8]) -> Vec<bool> {
    let mut bools = vec![];

    for i in 0..KEY_COUNT {
        bools.push(states[i + 1] != 0);
    }

    bools
}

/// Converts opendeck key index to device key index
pub fn opendeck_to_device(key: u8) -> u8 {
    if key < KEY_COUNT as u8 {
        [12, 9, 6, 3, 0, 15, 13, 10, 7, 4, 1, 16, 14, 11, 8, 5, 2, 17][key as usize]
    } else {
        key
    }
}

/// Converts device key index to opendeck key index
pub fn device_to_opendeck(key: usize) -> usize {
    let key = key - 1; // We have to subtract 1 from key index reported by device, because list is shifted by 1

    if key < KEY_COUNT {
        [4, 10, 16, 3, 9, 15, 2, 8, 14, 1, 7, 13, 0, 6, 12, 5, 11, 17][key]
    } else {
        key
    }
}

fn read_button_press(input: u8, state: u8) -> Result<DeviceInput, MirajazzError> {
    let mut button_states = vec![0x01];
    button_states.extend(vec![0u8; KEY_COUNT + 1]);

    if input == 0 {
        return Ok(DeviceInput::ButtonStateChange(read_button_states(
            &button_states,
        )));
    }

    let pressed_index: usize = device_to_opendeck(input as usize);

    // `device_to_opendeck` is 0-based, so add 1
    // I'll probably have to refactor all of this off-by-one stuff in this file, but that's a future me problem
    button_states[pressed_index + 1] = state;

    Ok(DeviceInput::ButtonStateChange(read_button_states(
        &button_states,
    )))
}

/// Processes the extended press/release reports emitted by patched HSV293S
/// firmware while remaining compatible with stock release-only firmware.
///
/// This is deliberately stateful: each HSV293S packet describes one key, not
/// the complete keyboard. Keeping the state here prevents pressing a second
/// key from implicitly releasing the first one.
pub fn process_hsv293s_report(
    data: &[u8],
    button_states: &mut [bool],
) -> Result<Vec<DeviceStateUpdate>, MirajazzError> {
    const BUTTON_REPORT_PREFIX: [u8; 9] = [0x41, 0x43, 0x4b, 0x00, 0x00, 0x4f, 0x4b, 0x00, 0x00];

    if data.len() < 11 || !data.starts_with(&BUTTON_REPORT_PREFIX) {
        return Ok(vec![]);
    }

    let input = data[9];
    if input == 0 {
        return Ok(vec![]);
    }
    if input as usize > KEY_COUNT || data[10] > 1 {
        return Err(MirajazzError::BadData);
    }

    let key = device_to_opendeck(input as usize);
    let Some(current_state) = button_states.get_mut(key) else {
        return Err(MirajazzError::BadData);
    };

    match (data[10], *current_state) {
        // Patched firmware: a real key-down packet.
        (1, false) => {
            *current_state = true;
            Ok(vec![DeviceStateUpdate::ButtonDown(key as u8)])
        }
        // Patched firmware: release the key that is currently held.
        (0, true) => {
            *current_state = false;
            Ok(vec![DeviceStateUpdate::ButtonUp(key as u8)])
        }
        // Stock firmware only reports a zero-state packet on release. Preserve
        // the old click behaviour when no preceding key-down was observed.
        (0, false) => Ok(vec![
            DeviceStateUpdate::ButtonDown(key as u8),
            DeviceStateUpdate::ButtonUp(key as u8),
        ]),
        // Ignore a duplicate down report.
        (1, true) => Ok(vec![]),
        _ => unreachable!(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report(input: u8, state: u8) -> [u8; 512] {
        let mut data = [0u8; 512];
        data[..9].copy_from_slice(&[0x41, 0x43, 0x4b, 0x00, 0x00, 0x4f, 0x4b, 0x00, 0x00]);
        data[9] = input;
        data[10] = state;
        data
    }

    #[test]
    fn emits_distinct_down_and_up_for_patched_firmware() {
        let mut states = vec![false; KEY_COUNT];

        assert!(matches!(
            process_hsv293s_report(&report(1, 1), &mut states).unwrap()[..],
            [DeviceStateUpdate::ButtonDown(4)]
        ));
        assert!(matches!(
            process_hsv293s_report(&report(1, 0), &mut states).unwrap()[..],
            [DeviceStateUpdate::ButtonUp(4)]
        ));
    }

    #[test]
    fn preserves_other_held_keys() {
        let mut states = vec![false; KEY_COUNT];

        process_hsv293s_report(&report(1, 1), &mut states).unwrap();
        assert!(matches!(
            process_hsv293s_report(&report(2, 1), &mut states).unwrap()[..],
            [DeviceStateUpdate::ButtonDown(10)]
        ));
        assert!(states[4]);
        assert!(states[10]);

        assert!(matches!(
            process_hsv293s_report(&report(1, 0), &mut states).unwrap()[..],
            [DeviceStateUpdate::ButtonUp(4)]
        ));
        assert!(!states[4]);
        assert!(states[10]);
    }

    #[test]
    fn keeps_stock_release_only_firmware_compatible() {
        let mut states = vec![false; KEY_COUNT];

        assert!(matches!(
            process_hsv293s_report(&report(1, 0), &mut states).unwrap()[..],
            [
                DeviceStateUpdate::ButtonDown(4),
                DeviceStateUpdate::ButtonUp(4)
            ]
        ));
    }
}
