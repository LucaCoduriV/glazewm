use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};

use anyhow::Context;
use tracing::info;
use windows::Win32::UI::Input::KeyboardAndMouse::VK_LWIN;
use crate::{
  common::{
    platform::{MouseMoveEvent, Platform},
    Point,
  },
  containers::{commands::set_focused_descendant, traits::CommonGetters},
  user_config::UserConfig,
  wm_state::WmState,
};
use crate::common::{Direction, Rect};
use crate::user_config::FloatingStateConfig;
use crate::windows::traits::WindowGetters;
use crate::windows::{ActiveDrag, WindowState};
use crate::windows::commands::update_window_state;

pub fn handle_mouse_move(
  event: MouseMoveEvent,
  state: &mut WmState,
  config: &UserConfig,
) -> anyhow::Result<()> {
  handle_alt_snap(event.clone(), state, config)?;

  handle_focus_on_hover(event, state, config)
}

// TODO: add these statics into the state instead
static MOUSE_X_POSITION: AtomicI32 = AtomicI32::new(i32::MAX);
static MOUSE_Y_POSITION: AtomicI32 = AtomicI32::new(i32::MAX);
static ALREADY_SET: AtomicBool = AtomicBool::new(false);
fn handle_alt_snap(
  event: MouseMoveEvent,
  state: &mut WmState,
  config: &UserConfig,
) -> anyhow::Result<()> {
  let old_mouse_x_positon = MOUSE_X_POSITION.load(Ordering::SeqCst);
  let old_mouse_y_positon = MOUSE_Y_POSITION.load(Ordering::SeqCst);

  if Platform::is_key_pressed(VK_LWIN) && event.is_mouse_down {
    let delta_x = event.point.x - old_mouse_x_positon;
    let delta_y = event.point.y - old_mouse_y_positon;


    let native_window = Platform::window_from_point(&event.point)?;

    let frame = native_window.refresh_frame_position()?;
    let window = state
      .window_from_native(&native_window)
      .context("window could not be found")?;

    let frame = frame.translate_in_direction(&Direction::Right, delta_x);
    let frame = frame.translate_in_direction(&Direction::Down, delta_y);

    if !ALREADY_SET.load(Ordering::SeqCst) {
      update_window_state(
        window,
        WindowState::Floating(FloatingStateConfig {
          centered: false,
          shown_on_top: true,
        }),
        state,
        config,
      )?;
    }


    info!("{:?}", &frame);

    let window = state
        .window_from_native(&native_window)
        .context("window could not be found")?;
    
    window.set_floating_placement(frame);

    // window.set_active_drag(Some(ActiveDrag {
    //   operation: None,
    //   is_from_tiling: window.is_tiling_window(),
    // }));
    ALREADY_SET.store(true, Ordering::SeqCst);
    state.pending_sync.focus_change = true;
    state.pending_sync.containers_to_redraw.push(window.into());
  }

  MOUSE_X_POSITION.store(event.point.x, Ordering::SeqCst);
  MOUSE_Y_POSITION.store(event.point.y, Ordering::SeqCst);
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
