use mirajazz::{error::MirajazzError, types::DeviceInput};

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

#[cfg(test)]
mod tests {
    use super::*;

    // The two translation tables must be strict inverses of each other over 0..KEY_COUNT.
    // opendeck_to_device maps OpenDeck index → device index (0-based).
    // device_to_opendeck maps device index → OpenDeck index, where device reports 1-based,
    // so device_to_opendeck(opendeck_to_device(k) + 1) must round-trip back to k.
    #[test]
    fn index_translation_round_trips() {
        for opendeck_idx in 0..KEY_COUNT as u8 {
            let device_idx = opendeck_to_device(opendeck_idx);
            // Device reports 1-based indices, so add 1 before passing to device_to_opendeck
            let back = device_to_opendeck(device_idx as usize + 1);
            assert_eq!(
                back, opendeck_idx as usize,
                "round-trip failed for OpenDeck index {opendeck_idx}: \
                 opendeck_to_device={device_idx}, device_to_opendeck({})={back}",
                device_idx + 1
            );
        }
    }

    // opendeck_to_device must produce exactly the set {0..KEY_COUNT} — a permutation, no gaps.
    #[test]
    fn opendeck_to_device_is_permutation() {
        let mut seen = vec![false; KEY_COUNT];
        for k in 0..KEY_COUNT as u8 {
            let d = opendeck_to_device(k) as usize;
            assert!(d < KEY_COUNT, "opendeck_to_device({k}) = {d} out of range");
            assert!(!seen[d], "opendeck_to_device produced duplicate index {d}");
            seen[d] = true;
        }
    }

    // Keys outside the valid range must pass through unchanged.
    #[test]
    fn out_of_range_keys_pass_through() {
        for k in KEY_COUNT as u8..=255u8 {
            assert_eq!(opendeck_to_device(k), k);
        }
        for k in KEY_COUNT + 1..=255 {
            // device_to_opendeck subtracts 1 internally; key=KEY_COUNT+1 → 0-based KEY_COUNT which is out of range
            assert_eq!(device_to_opendeck(k + 1), k);
        }
    }

    // input=0 means "all buttons released" — the returned state must be all-false.
    #[test]
    fn process_input_zero_returns_all_released() {
        let result = process_input(0, 0).unwrap();
        if let DeviceInput::ButtonStateChange(states) = result {
            assert_eq!(states.len(), KEY_COUNT);
            assert!(states.iter().all(|&s| !s), "expected all buttons released");
        } else {
            panic!("expected ButtonStateChange");
        }
    }

    // Each valid device button index (1..=KEY_COUNT) with state=1 must produce exactly
    // one true entry in the returned state vec, at the correct OpenDeck position.
    #[test]
    fn process_input_single_button_press() {
        for device_btn in 1u8..=KEY_COUNT as u8 {
            let result = process_input(device_btn, 1).unwrap();
            if let DeviceInput::ButtonStateChange(states) = result {
                assert_eq!(states.len(), KEY_COUNT);
                let pressed: Vec<usize> = states.iter().enumerate()
                    .filter(|&(_, &s)| s)
                    .map(|(i, _)| i)
                    .collect();
                assert_eq!(
                    pressed.len(), 1,
                    "device button {device_btn} produced {} pressed entries (expected 1)",
                    pressed.len()
                );
                let expected_opendeck = device_to_opendeck(device_btn as usize);
                assert_eq!(
                    pressed[0], expected_opendeck,
                    "device button {device_btn}: expected OpenDeck index {expected_opendeck}, got {}",
                    pressed[0]
                );
            } else {
                panic!("expected ButtonStateChange for device_btn={device_btn}");
            }
        }
    }

    // state=0 (release) on a valid button must produce all-false state.
    #[test]
    fn process_input_button_release_all_false() {
        for device_btn in 1u8..=KEY_COUNT as u8 {
            let result = process_input(device_btn, 0).unwrap();
            if let DeviceInput::ButtonStateChange(states) = result {
                assert!(
                    states.iter().all(|&s| !s),
                    "device button {device_btn} release should produce all-false"
                );
            } else {
                panic!("expected ButtonStateChange");
            }
        }
    }

    // An input index beyond KEY_COUNT must be rejected with BadData.
    #[test]
    fn process_input_out_of_range_is_bad_data() {
        let result = process_input(KEY_COUNT as u8 + 1, 1);
        assert!(
            matches!(result, Err(MirajazzError::BadData)),
            "expected BadData for out-of-range input"
        );
    }
}
