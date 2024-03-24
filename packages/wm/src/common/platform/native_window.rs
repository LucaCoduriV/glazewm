use std::sync::{Arc, RwLock};

use anyhow::Result;
use windows::{
  core::PWSTR,
  Win32::{
    Foundation::{CloseHandle, BOOL, HWND, LPARAM},
    Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED},
    System::Threading::{
      OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
      PROCESS_QUERY_INFORMATION,
    },
    UI::{
      Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT,
        KEYBD_EVENT_FLAGS, VIRTUAL_KEY,
      },
      WindowsAndMessaging::{
        EnumWindows, GetClassNameW, GetWindow, GetWindowLongPtrW,
        GetWindowPlacement, GetWindowTextW, GetWindowThreadProcessId,
        IsWindowVisible, SetForegroundWindow, GWL_EXSTYLE, GWL_STYLE,
        GW_OWNER, WINDOWPLACEMENT, WINDOW_EX_STYLE, WINDOW_STYLE,
        WS_CAPTION, WS_CHILD, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
      },
    },
  },
};

pub type WindowHandle = HWND;

#[derive(Debug)]
pub struct NativeWindow {
  pub handle: WindowHandle,
  title: Arc<RwLock<Option<String>>>,
  process_name: Arc<RwLock<Option<String>>>,
  class_name: Arc<RwLock<Option<String>>>,
}

impl NativeWindow {
  pub fn new(handle: WindowHandle) -> Self {
    Self {
      handle,
      title: Arc::new(RwLock::new(None)),
      process_name: Arc::new(RwLock::new(None)),
      class_name: Arc::new(RwLock::new(None)),
    }
  }

  /// Gets the window's title. If the window is invalid, returns an empty
  /// string.
  ///
  /// This value is lazily retrieved and is cached after first retrieval.
  pub fn title(&self) -> String {
    let title_guard = self.title.read().unwrap();
    match *title_guard {
      Some(ref title) => title.clone(),
      None => {
        let mut text: [u16; 512] = [0; 512];
        let length = unsafe { GetWindowTextW(self.handle, &mut text) };

        let title = String::from_utf16_lossy(&text[..length as usize]);
        *self.title.write().unwrap() = Some(title.clone());
        title
      }
    }
  }

  /// Gets the process name associated with the window.
  ///
  /// This value is lazily retrieved and is cached after first retrieval.
  pub fn process_name(&self) -> Result<String> {
    let process_name_guard = self.process_name.read().unwrap();
    match *process_name_guard {
      Some(ref process_name) => Ok(process_name.clone()),
      None => {
        let mut process_id = 0u32;
        unsafe {
          GetWindowThreadProcessId(self.handle, Some(&mut process_id));
        }

        let process_handle = unsafe {
          OpenProcess(PROCESS_QUERY_INFORMATION, false, process_id)
        }?;

        let mut buffer = [0u16; 256];
        let mut length = buffer.len() as u32;
        unsafe {
          QueryFullProcessImageNameW(
            process_handle,
            PROCESS_NAME_WIN32,
            PWSTR(buffer.as_mut_ptr()),
            &mut length,
          )?;

          CloseHandle(process_handle)?;
        };

        let process_name = String::from_utf16(&buffer[..length as usize])?;
        *self.process_name.write().unwrap() = Some(process_name.clone());
        Ok(process_name)
      }
    }
  }

  /// Gets the class name of the window.
  ///
  /// This value is lazily retrieved and is cached after first retrieval.
  pub fn class_name(&self) -> Result<String> {
    let class_name_guard = self.class_name.read().unwrap();
    match *class_name_guard {
      Some(ref class_name) => Ok(class_name.clone()),
      None => {
        let mut buffer = [0u16; 256];
        let result = unsafe { GetClassNameW(self.handle, &mut buffer) };

        if result == 0 {
          return Err(windows::core::Error::from_win32().into());
        }

        let class_name =
          String::from_utf16_lossy(&buffer[..result as usize]);
        *self.class_name.write().unwrap() = Some(class_name.clone());
        Ok(class_name)
      }
    }
  }

  /// Whether the window is actually visible.
  pub fn is_visible(&self) -> bool {
    let is_visible = unsafe { IsWindowVisible(self.handle) }.as_bool();
    is_visible && !self.is_cloaked()
  }

  /// Whether the window is cloaked. For some UWP apps, `WS_VISIBLE` will
  /// be present even if the window isn't actually visible. The
  /// `DWMWA_CLOAKED` attribute is used to check whether these apps are
  /// visible.
  fn is_cloaked(&self) -> bool {
    let mut cloaked = 0u32;

    let _ = unsafe {
      DwmGetWindowAttribute(
        self.handle,
        DWMWA_CLOAKED,
        &mut cloaked as *mut u32 as _,
        std::mem::size_of::<u32>() as u32,
      )
    };

    cloaked != 0
  }

  pub fn is_manageable(&self) -> bool {
    // Ignore windows that are hidden.
    if !self.is_visible() {
      return false;
    }

    // Ensure window is top-level (ie. not a child window). Ignore windows
    // that cannot be focused or if they're unavailable in task switcher
    // (alt+tab menu).
    let is_application_window = !self.has_window_style(WS_CHILD)
      && !self.has_window_style_ex(WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW);

    if !is_application_window {
      return false;
    }

    // Some applications spawn top-level windows for menus that should be
    // ignored. This includes the autocomplete popup in Notepad++ and title
    // bar menu in Keepass. Although not foolproof, these can typically be
    // identified by having an owner window and no title bar.
    let is_menu_window = unsafe { GetWindow(self.handle, GW_OWNER) }.0
      != 0
      && !self.has_window_style(WS_CAPTION);

    !is_menu_window
  }

  pub fn is_minimized(&self) -> bool {
    todo!()
  }

  pub fn is_maximized(&self) -> bool {
    todo!()
  }

  pub fn is_resizable(&self) -> bool {
    todo!()
  }

  pub fn is_app_bar(&self) -> bool {
    todo!()
  }

  pub fn set_foreground(&self) -> anyhow::Result<()> {
    // Simulate a key press event to activate the window.
    let input = INPUT {
      r#type: INPUT_KEYBOARD,
      Anonymous: INPUT_0 {
        ki: KEYBDINPUT {
          wVk: VIRTUAL_KEY(0),
          wScan: 0,
          dwFlags: KEYBD_EVENT_FLAGS(0),
          time: 0,
          dwExtraInfo: 0,
        },
      },
    };

    unsafe {
      SendInput(&[input], std::mem::size_of::<INPUT>() as i32);
    }

    // Set as the foreground window.
    unsafe { SetForegroundWindow(self.handle) }.ok()?;

    Ok(())
  }

  fn size(&self) -> (i32, i32) {
    let mut placement = WINDOWPLACEMENT::default();
    let _ = unsafe { GetWindowPlacement(self.handle, &mut placement) };
    let rect = placement.rcNormalPosition;
    ((rect.right - rect.left), (rect.bottom - rect.top))
  }

  fn has_window_style(&self, style: WINDOW_STYLE) -> bool {
    unsafe {
      let current_style = GetWindowLongPtrW(self.handle, GWL_STYLE);
      (current_style & style.0 as isize) != 0
    }
  }

  fn has_window_style_ex(&self, style: WINDOW_EX_STYLE) -> bool {
    unsafe {
      let current_style = GetWindowLongPtrW(self.handle, GWL_EXSTYLE);
      (current_style & style.0 as isize) != 0
    }
  }
}

impl PartialEq for NativeWindow {
  fn eq(&self, other: &Self) -> bool {
    self.handle == other.handle
  }
}

impl Eq for NativeWindow {}

pub fn available_windows() -> Result<Vec<NativeWindow>> {
  available_window_handles()?
    .into_iter()
    .map(|handle| Ok(NativeWindow::new(handle)))
    .collect()
}

pub fn available_window_handles() -> anyhow::Result<Vec<WindowHandle>> {
  let mut handles: Vec<WindowHandle> = Vec::new();

  unsafe {
    EnumWindows(
      Some(available_window_handles_proc),
      LPARAM(&mut handles as *mut _ as _),
    )
  }?;

  Ok(handles)
}

extern "system" fn available_window_handles_proc(
  handle: HWND,
  data: LPARAM,
) -> BOOL {
  let handles = data.0 as *mut Vec<HWND>;
  unsafe { (*handles).push(handle) };
  true.into()
}
