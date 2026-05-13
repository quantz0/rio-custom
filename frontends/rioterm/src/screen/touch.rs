// Part of this file was originally taken from Alacritty
// https://github.com/alacritty/alacritty/blob/34b5dbacd28cd1abaedf1d81cc0ebe57aa44a086/alacritty/src/input.rs
// which is licensed under Apache 2.0 license.

use std::collections::hash_map::RandomState;
use std::time::Instant;
use std::{collections::HashSet, mem};

use rio_window::event::{ElementState, MouseButton, Touch, TouchPhase};

use crate::event::ClickState;
use crate::router::Route;

#[derive(Debug, Default)]
pub enum TouchPurpose {
    #[default]
    None,
    Select(Touch),
    Scroll(Touch),
    Tap(Touch),
    Invalid(HashSet<u64, RandomState>),
}

/// Distance before a touch input is considered a drag.
pub const MAX_TAP_DISTANCE: f64 = 5.;

#[inline]
pub fn on_touch(
    route: &mut Route,
    touch: Touch,
    clipboard: &mut rio_backend::clipboard::Clipboard,
) {
    match touch.phase {
        TouchPhase::Started => {
            on_touch_start(route, touch, clipboard);
        }
        TouchPhase::Moved => on_touch_motion(route, touch, clipboard),
        TouchPhase::Ended | TouchPhase::Cancelled => {
            on_touch_end(route, touch, clipboard)
        }
    }
}

#[inline]
fn on_touch_start(
    route: &mut Route,
    touch: Touch,
    _clipboard: &mut rio_backend::clipboard::Clipboard,
) {
    let touch_purpose = route.window.screen.touch_purpose();
    *touch_purpose = match mem::take(touch_purpose) {
        TouchPurpose::None => TouchPurpose::Tap(touch),
        TouchPurpose::Scroll(event) | TouchPurpose::Select(event) => {
            let mut set = HashSet::default();
            set.insert(event.id);
            TouchPurpose::Invalid(set)
        }
        TouchPurpose::Tap(start) => {
            let mut set = HashSet::default();
            set.insert(start.id);
            set.insert(touch.id);
            TouchPurpose::Invalid(set)
        }
        TouchPurpose::Invalid(mut slots) => {
            slots.insert(touch.id);
            TouchPurpose::Invalid(slots)
        }
    };
}

#[inline]
fn on_touch_motion(
    route: &mut Route,
    touch: Touch,
    clipboard: &mut rio_backend::clipboard::Clipboard,
) {
    let touch_purpose = route.window.screen.touch_purpose();
    match touch_purpose {
        TouchPurpose::None => (),
        // Handle transition from tap to scroll/select.
        TouchPurpose::Tap(start) => {
            let delta_x = touch.location.x - start.location.x;
            let delta_y = touch.location.y - start.location.y;
            if delta_x.abs() > MAX_TAP_DISTANCE {
                tracing::info!("tap to select");
                // Update gesture state.
                let start_location = start.location;
                *touch_purpose = TouchPurpose::Select(*start);

                let layout = route.window.screen.sugarloaf.window_size();

                // Start simulated mouse input.
                let x = start_location.x.clamp(0.0, layout.width.into());
                let y = start_location.y.clamp(0.0, layout.height.into());

                route.window.screen.mouse.x = x;
                route.window.screen.mouse.y = y;

                let now = Instant::now();
                route.window.screen.mouse.last_click_timestamp = now;
                route.window.screen.mouse.last_click_button = MouseButton::Left;

                route.window.screen.mouse.click_state = ClickState::Click;
                route.window.screen.mouse.left_button_state = ElementState::Pressed;
                route
                    .window
                    .screen
                    .on_left_click(route.window.screen.mouse_position(0), clipboard);

                // Apply motion since touch start.
                on_touch_motion(route, touch, clipboard);
            } else if delta_y.abs() > MAX_TAP_DISTANCE {
                tracing::info!("tap to scroll");
                // Update gesture state.
                *touch_purpose = TouchPurpose::Scroll(*start);
                // Apply motion since touch start.
                on_touch_motion(route, touch, clipboard);
            } else {
                tracing::info!("tap normal");
            }
        }
        TouchPurpose::Scroll(last_touch) => {
            // Calculate delta and update last touch position.
            let delta_y = touch.location.y - last_touch.location.y;
            *touch_purpose = TouchPurpose::Scroll(touch);
            route.window.screen.scroll(0., delta_y);
            tracing::info!("scroll motion: {}", delta_y);
        }
        TouchPurpose::Select(_) => {
            let layout = route.window.screen.sugarloaf.window_size();
            let x = touch.location.x.clamp(0.0, layout.width.into());
            let y = touch.location.y.clamp(0.0, layout.height.into());
            route.window.screen.mouse.x = x;
            route.window.screen.mouse.y = y;
            tracing::info!("select motion");
        }
        TouchPurpose::Invalid(_) => (),
    }
}

#[inline]
fn on_touch_end(
    route: &mut Route,
    touch: Touch,
    clipboard: &mut rio_backend::clipboard::Clipboard,
) {
    on_touch_motion(route, touch, clipboard);

    let touch_purpose = route.window.screen.touch_purpose();
    match touch_purpose {
        // Simulate LMB clicks.
        TouchPurpose::Tap(start) => {
            let start_location = start.location;
            *touch_purpose = Default::default();

            let layout = route.window.screen.sugarloaf.window_size();

            let x = start_location.x.clamp(0.0, layout.width.into());
            let y = start_location.y.clamp(0.0, layout.height.into());

            route.window.screen.mouse.x = x;
            route.window.screen.mouse.y = y;

            let now = Instant::now();
            route.window.screen.mouse.last_click_timestamp = now;
            route.window.screen.mouse.last_click_button = MouseButton::Left;

            route.window.screen.mouse.click_state = ClickState::Click;
            route.window.screen.mouse.left_button_state = ElementState::Pressed;
            route
                .window
                .screen
                .on_left_click(route.window.screen.mouse_position(0), clipboard);
            route.window.screen.mouse.click_state = ClickState::None;
            route.window.screen.mouse.left_button_state = ElementState::Released;
            tracing::info!("tap end");
        }
        // Reset touch state once all slots were released.
        TouchPurpose::Invalid(slots) => {
            slots.remove(&touch.id);
            if slots.is_empty() {
                *touch_purpose = Default::default();
            }
        }
        // Release simulated LMB.
        TouchPurpose::Select(_) => {
            *touch_purpose = Default::default();
            route.window.screen.mouse.click_state = ClickState::None;
            route.window.screen.mouse.left_button_state = ElementState::Released;
            tracing::info!("select end");
        }
        // Reset touch state on scroll finish.
        TouchPurpose::Scroll(_) => {
            *touch_purpose = Default::default();
            tracing::info!("scroll end");
        }
        TouchPurpose::None => (),
    }
}
