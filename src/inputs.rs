use mirajazz::{error::MirajazzError, types::DeviceInput};

pub fn process_input_with_mapping(
    key_count: usize,
    device_to_opendeck: impl Fn(usize) -> usize,
    input: u8,
    state: u8,
) -> Result<DeviceInput, MirajazzError> {
    log::info!("Processing input: {}, {}", input, state);

    match input as usize {
        0..=255 => read_button_press(key_count, device_to_opendeck, input, state),
        _ => Err(MirajazzError::BadData),
    }
}

fn read_button_states(states: &[u8], key_count: usize) -> Vec<bool> {
    let mut bools = vec![];

    for i in 0..key_count {
        bools.push(states[i + 1] != 0);
    }

    bools
}

fn read_button_press(
    key_count: usize,
    device_to_opendeck: impl Fn(usize) -> usize,
    input: u8,
    state: u8,
) -> Result<DeviceInput, MirajazzError> {
    let mut button_states = vec![0x01];
    button_states.extend(vec![0u8; key_count]);

    if input == 0 {
        return Ok(DeviceInput::ButtonStateChange(read_button_states(
            &button_states,
            key_count,
        )));
    }

    let pressed_index = device_to_opendeck(input as usize);
    if pressed_index >= key_count {
        return Err(MirajazzError::BadData);
    }

    // `device_to_opendeck` is 0-based, so add 1
    // I'll probably have to refactor all of this off-by-one stuff in this file, but that's a future me problem
    button_states[pressed_index + 1] = state;

    Ok(DeviceInput::ButtonStateChange(read_button_states(
        &button_states,
        key_count,
    )))
}
