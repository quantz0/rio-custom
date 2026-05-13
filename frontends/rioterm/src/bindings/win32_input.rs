use rio_window::event::{ElementState, KeyEvent};
use rio_window::keyboard::Key;
use rio_window::platform::modifier_supplement::KeyEventExtModifierSupplement;
use rio_window::platform::windows::KeyEventExtWindows;

pub fn build_key_sequence(key: &KeyEvent) -> Vec<u8> {
    let event = key.win32_key_event();
    let unicode_char = unicode_char(key);
    let key_down = match key.state {
        ElementState::Pressed => 1,
        ElementState::Released => 0,
    };

    format!(
        "\x1b[{};{};{};{};{};{}_",
        event.virtual_key_code,
        event.virtual_scan_code,
        unicode_char,
        key_down,
        event.control_key_state,
        event.repeat_count,
    )
    .into_bytes()
}

fn unicode_char(key: &KeyEvent) -> u16 {
    key.text_with_all_modifiers()
        .and_then(first_utf16)
        .or_else(|| key.logical_key.to_text().and_then(first_utf16))
        .or_else(|| match key.key_without_modifiers() {
            Key::Character(text) => first_utf16(&text),
            _ => None,
        })
        .unwrap_or(0)
}

fn first_utf16(text: &str) -> Option<u16> {
    text.encode_utf16().next()
}
