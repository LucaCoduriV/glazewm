use std::{
  sync::atomic::{AtomicBool, AtomicI32, Ordering},
  time::{Duration, Instant},
};

use anyhow::Context;
use tracing::info;
use windows::Win32::{
  Foundation::{HWND, RECT},
  Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_EXTENDED_FRAME_BOUNDS},
  UI::{
    Input::KeyboardAndMouse::VK_LWIN,
    WindowsAndMessaging::{
      SetWindowPos, SWP_ASYNCWINDOWPOS, SWP_NOSENDCHANGING, SWP_NOSIZE,
      SWP_NOZORDER,
    },
  },
};

use crate::{
  common::{
    platform::{MouseMoveEvent, Platform},
    Direction, Point, Rect,
  },
  containers::{commands::set_focused_descendant, traits::CommonGetters},
  user_config::{FloatingStateConfig, UserConfig},
  windows::{
    commands::update_window_state, traits::WindowGetters, ActiveDrag,
    WindowState,
  },
  wm_state::{AltSnap, WmState},
};

pub fn handle_mouse_move(
  event: MouseMoveEvent,
  state: &mut WmState,
  config: &UserConfig,
) -> anyhow::Result<()> {
  handle_focus_on_hover(event, state, config)
}

// TODO: add these statics into the state instead
pub fn handle_alt_snap(
  event: MouseMoveEvent,
  state: &mut AltSnap,
) -> anyhow::Result<()> {
  // if Platform::is_key_pressed(VK_LWIN) && event.is_mouse_down {
  if event.is_mouse_down {
    // let old_instant =
    //   state.alt_snap.last_move_time.get_or_insert(Instant::now());
    //
    // if old_instant.elapsed() <= Duration::from_millis(10) {
    //   return Ok(());
    // } else {
    //   state.alt_snap.last_move_time = None;
    // }

    let old_mouse_pos = state
      .old_mouse_position
      .clone()
      .unwrap_or(event.point.clone());

    let delta_mouse_pos = Point {
      x: event.point.x - old_mouse_pos.x,
      y: event.point.y - old_mouse_pos.y,
    };

    let native_window = Platform::window_from_point(&event.point)?;

    // let window = state
    //   .window_from_native(&native_window)
    //   .context("window could not be found")?;

    let mut rect = RECT::default();

    unsafe {
      DwmGetWindowAttribute(
        HWND(native_window.handle),
        DWMWA_EXTENDED_FRAME_BOUNDS,
        &mut rect as *mut _ as _,
        std::mem::size_of::<RECT>() as u32,
      )?;
    }
    let frame =
      Rect::from_ltrb(rect.left, rect.top, rect.right, rect.bottom);

    // let frame =
    //   frame.translate_in_direction(&Direction::Right,
    // delta_mouse_pos.x); let frame =
    //   frame.translate_in_direction(&Direction::Down, delta_mouse_pos.y);

    // if !state.alt_snap.is_currently_moving {
    //   update_window_state(
    //     window,
    //     WindowState::Floating(FloatingStateConfig {
    //       centered: false,
    //       shown_on_top: true,
    //     }),
    //     state,
    //     config,
    //   )?;
    // }

    // let window = state
    //   .window_from_native(&native_window)
    //   .context("window could not be found")?;

    // window.set_floating_placement(frame.clone());

    // window.set_active_drag(Some(ActiveDrag {
    //   operation: None,
    //   is_from_tiling: window.is_tiling_window(),
    // }));
    state.is_currently_moving = true;

    // TODO: refactor this. Using windows call directly removes some of
    // stutters
    unsafe {
      SetWindowPos(
        HWND(native_window.handle),
        HWND::default(),
        event.point.x - 500,
        event.point.y - 500,
        0,
        0,
        SWP_NOSIZE
          | SWP_NOZORDER
          | SWP_NOSENDCHANGING
          | SWP_ASYNCWINDOWPOS,
      )?;
    }
    // state.pending_sync.focus_change = true;
    // state.pending_sync.containers_to_redraw.push(window.into());
  }

  state.old_mouse_position = Some(Point {
    x: event.point.x,
    y: event.point.y,
  });
  Ok(())
}

fn handle_focus_on_hover(
  event: MouseMoveEvent,
  state: &mut WmState,
  config: &UserConfig,
) -> anyhow::Result<()> {
  // Ignore event if left/right-click is down. Otherwise, this causes focus
  // to jitter when a window is being resized by its drag handles.
  if event.is_mouse_down || !config.value.general.focus_follows_cursor {
    return Ok(());
  }

  let window_under_cursor = Platform::window_from_point(&event.point)
    .and_then(|window| Platform::root_ancestor(&window))
    .map(|root| state.window_from_native(&root))?;

  // Set focus to whichever window is currently under the cursor.
  if let Some(window) = window_under_cursor {
    let focused_container =
      state.focused_container().context("No focused container.")?;

    if focused_container.id() != window.id() {
      set_focused_descendant(window.as_container(), None);
      state.pending_sync.focus_change = true;
    }
  }

  Ok(())
}
