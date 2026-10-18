//! Action execution: turns an [`Action`] into platform calls.
//!
//! The engine is generic over [`WindowSystem`], so all of the behaviour below is
//! unit tested against an in-memory mock and the platform crates only contain
//! FFI glue.

use crate::action::Action;
use crate::config::Config;
use crate::display::Displays;
use crate::geometry::Rect;
use crate::tiling;
use crate::window::{Permission, PlatformError, WindowId, WindowSystem};
use std::fmt;

/// Frames closer than this are considered identical, so a repeated hotkey press
/// does not keep re-applying an identical `SetWindowPos`.
pub const FRAME_TOLERANCE: f64 = 1.0;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EngineError {
    /// The platform reported no displays at all.
    NoDisplays,
    Platform(PlatformError),
}

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EngineError::NoDisplays => f.write_str("没有检测到可用显示器"),
            EngineError::Platform(err) => err.fmt(f),
        }
    }
}

impl std::error::Error for EngineError {}

impl From<PlatformError> for EngineError {
    fn from(err: PlatformError) -> Self {
        EngineError::Platform(err)
    }
}

/// What an action actually did.
#[derive(Clone, Debug, PartialEq)]
pub enum Outcome {
    Applied {
        action: Action,
        window: WindowId,
        /// `None` for actions that do not reposition (e.g. minimize).
        frame: Option<Rect>,
    },
    NoOp {
        action: Action,
        reason: String,
    },
}

impl Outcome {
    pub fn action(&self) -> Action {
        match self {
            Outcome::Applied { action, .. } | Outcome::NoOp { action, .. } => *action,
        }
    }

    pub fn status_message(&self, label: &str) -> String {
        match self {
            Outcome::Applied {
                frame: Some(frame), ..
            } => format!("{label} · {frame}"),
            Outcome::Applied { frame: None, .. } => label.to_string(),
            Outcome::NoOp { reason, .. } => format!("{label}：{reason}"),
        }
    }
}

/// Remembers the frame a window had before rect-rs moved it, so `restore` can go
/// back. Frames are kept per window and dropped once consumed.
#[derive(Debug)]
struct RestoreCache {
    entries: Vec<(WindowId, Rect)>,
    limit: usize,
}

impl Default for RestoreCache {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            limit: 128,
        }
    }
}

impl RestoreCache {
    fn remember(&mut self, window: WindowId, frame: Rect) {
        if let Some(entry) = self.entries.iter_mut().find(|(id, _)| *id == window) {
            entry.1 = frame;
            return;
        }
        if self.entries.len() >= self.limit {
            self.entries.remove(0);
        }
        self.entries.push((window, frame));
    }

    fn take(&mut self, window: WindowId) -> Option<Rect> {
        let index = self.entries.iter().position(|(id, _)| *id == window)?;
        Some(self.entries.remove(index).1)
    }
}

pub struct Engine<S: WindowSystem> {
    system: S,
    config: Config,
    restore: RestoreCache,
}

impl<S: WindowSystem> Engine<S> {
    pub fn new(system: S, config: Config) -> Self {
        Self {
            system,
            config,
            restore: RestoreCache::default(),
        }
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn set_config(&mut self, config: Config) {
        self.config = config;
    }

    pub fn system(&self) -> &S {
        &self.system
    }

    pub fn platform_name(&self) -> &'static str {
        self.system.platform_name()
    }

    pub fn permission(&self) -> Permission {
        self.system.permission()
    }

    pub fn displays(&self) -> Result<Displays, EngineError> {
        self.system.displays().map_err(EngineError::from)
    }

    /// Remember the window the user is working in before rect-rs takes focus.
    pub fn remember_foreground(&self) {
        self.system.remember_foreground();
    }

    pub fn perform(&mut self, action: Action) -> Result<Outcome, EngineError> {
        if let Some(anchor) = action.move_anchor() {
            return self.place(action, |current, area, margin| {
                tiling::move_frame(anchor, area, current, margin)
            });
        }
        if let Some(layout) = action.tile_layout() {
            return self.place(action, |_current, area, margin| {
                tiling::tile_frame(layout, area, margin)
            });
        }

        match action {
            Action::Maximize => self.place(action, |_current, area, margin| {
                area.inset(margin).clamp_inside(&area).rounded()
            }),
            Action::Restore => self.restore(action),
            Action::Minimize => self.minimize(action),
            Action::NextDisplay => self.move_to_display(action, true),
            Action::PreviousDisplay => self.move_to_display(action, false),
            other => Err(EngineError::Platform(PlatformError::Unsupported(format!(
                "未实现的动作：{other}"
            )))),
        }
    }

    /// Resolve the window + its work area, compute a new frame and apply it.
    fn place(
        &mut self,
        action: Action,
        compute: impl FnOnce(Rect, Rect, f64) -> Rect,
    ) -> Result<Outcome, EngineError> {
        let window = self.system.focused_window()?;
        let current = self.checked_frame(window)?;
        let area = self.work_area_of(current)?;
        let target = compute(current, area, self.config.margin());

        if target.roughly_equals(&current, FRAME_TOLERANCE) {
            return Ok(Outcome::NoOp {
                action,
                reason: format!("窗口已在目标位置 {target}"),
            });
        }

        self.restore.remember(window, current);
        self.system.set_window_frame(window, target)?;
        Ok(Outcome::Applied {
            action,
            window,
            frame: Some(target),
        })
    }

    fn work_area_of(&self, current: Rect) -> Result<Rect, EngineError> {
        let displays = self.system.displays()?;
        Ok(displays
            .display_for(current)
            .ok_or(EngineError::NoDisplays)?
            .visible)
    }

    fn minimize(&mut self, action: Action) -> Result<Outcome, EngineError> {
        let window = self.system.focused_window()?;
        self.system.set_minimized(window, true)?;
        Ok(Outcome::Applied {
            action,
            window,
            frame: None,
        })
    }

    fn restore(&mut self, action: Action) -> Result<Outcome, EngineError> {
        let window = self.system.focused_window()?;
        let current = self.checked_frame(window)?;

        if let Some(previous) = self.restore.take(window) {
            if previous.roughly_equals(&current, FRAME_TOLERANCE) {
                return Ok(Outcome::NoOp {
                    action,
                    reason: "窗口已经是恢复后的尺寸".into(),
                });
            }
            self.system.set_window_frame(window, previous)?;
            return Ok(Outcome::Applied {
                action,
                window,
                frame: Some(previous),
            });
        }

        // No frame remembered: handle windows maximized by the OS itself by
        // shrinking them, which is what users expect from "cancel maximize".
        let displays = self.system.displays()?;
        let display = displays
            .display_for(current)
            .ok_or(EngineError::NoDisplays)?;
        if !tiling::fills_work_area(current, display.visible, 2.0) {
            return Ok(Outcome::NoOp {
                action,
                reason: "没有记录到可恢复的尺寸".into(),
            });
        }

        let target = tiling::restore_fallback_frame(current, display.visible, self.config.margin());
        self.system.set_window_frame(window, target)?;
        Ok(Outcome::Applied {
            action,
            window,
            frame: Some(target),
        })
    }

    /// Move the window to another display, keeping its size.
    fn move_to_display(&mut self, action: Action, forward: bool) -> Result<Outcome, EngineError> {
        let window = self.system.focused_window()?;
        let current = self.checked_frame(window)?;
        let displays = self.system.displays()?;

        if displays.len() < 2 {
            return Ok(Outcome::NoOp {
                action,
                reason: "只有一个显示器".into(),
            });
        }

        let source = displays
            .display_for(current)
            .ok_or(EngineError::NoDisplays)?
            .clone();
        let target_display = displays
            .neighbour_of(source.id, forward)
            .ok_or(EngineError::NoDisplays)?
            .clone();
        if target_display.id == source.id {
            return Ok(Outcome::NoOp {
                action,
                reason: "只有一个显示器".into(),
            });
        }

        let target =
            tiling::frame_on_target_display(current, source.visible, target_display.visible);
        if target.roughly_equals(&current, FRAME_TOLERANCE) {
            return Ok(Outcome::NoOp {
                action,
                reason: format!("窗口已在 {} 上", target_display.label()),
            });
        }

        self.restore.remember(window, current);
        self.system.set_window_frame(window, target)?;
        Ok(Outcome::Applied {
            action,
            window,
            frame: Some(target),
        })
    }

    fn checked_frame(&self, window: WindowId) -> Result<Rect, EngineError> {
        let frame = self.system.window_frame(window)?;
        if !frame.is_finite() || frame.is_empty() {
            return Err(EngineError::Platform(PlatformError::WindowUnavailable(
                format!("窗口尺寸异常（{frame}）"),
            )));
        }
        Ok(frame)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::display::{Display, DisplayId};
    use std::cell::RefCell;

    /// In-memory `WindowSystem`: one window, N displays, records applied frames.
    struct Mock {
        frame: RefCell<Rect>,
        displays: Vec<Display>,
        focused: Option<WindowId>,
        minimized: RefCell<bool>,
        applied: RefCell<Vec<Rect>>,
        permission: Permission,
    }

    impl Mock {
        fn with_displays(displays: Vec<Display>, frame: Rect) -> Self {
            Self {
                frame: RefCell::new(frame),
                displays,
                focused: Some(WindowId(42)),
                minimized: RefCell::new(false),
                applied: RefCell::new(Vec::new()),
                permission: Permission::Granted,
            }
        }

        fn primary_only() -> Self {
            Self::with_displays(
                vec![display(1, 0.0, 0.0, 1920.0, 1040.0, true)],
                Rect::new(300.0, 200.0, 800.0, 600.0),
            )
        }

        fn two_displays() -> Self {
            Self::with_displays(
                vec![
                    display(1, 0.0, 0.0, 1920.0, 1040.0, true),
                    display(2, 1920.0, 0.0, 1280.0, 720.0, false),
                ],
                Rect::new(560.0, 220.0, 800.0, 600.0),
            )
        }

        fn last_frame(&self) -> Rect {
            *self.frame.borrow()
        }

        fn apply_count(&self) -> usize {
            self.applied.borrow().len()
        }
    }

    fn display(id: u64, x: f64, y: f64, width: f64, height: f64, primary: bool) -> Display {
        Display::new(
            DisplayId(id),
            format!("DISPLAY{id}"),
            Rect::new(x, y, width, height),
            Rect::new(x, y, width, height),
        )
        .with_primary(primary)
    }

    impl WindowSystem for Mock {
        fn focused_window(&self) -> Result<WindowId, PlatformError> {
            self.focused.ok_or(PlatformError::NoFocusedWindow)
        }

        fn window_frame(&self, _window: WindowId) -> Result<Rect, PlatformError> {
            Ok(*self.frame.borrow())
        }

        fn set_window_frame(&self, _window: WindowId, frame: Rect) -> Result<(), PlatformError> {
            *self.frame.borrow_mut() = frame;
            self.applied.borrow_mut().push(frame);
            Ok(())
        }

        fn set_minimized(&self, _window: WindowId, minimized: bool) -> Result<(), PlatformError> {
            *self.minimized.borrow_mut() = minimized;
            Ok(())
        }

        fn displays(&self) -> Result<Displays, PlatformError> {
            Ok(Displays::new(self.displays.clone()))
        }

        fn permission(&self) -> Permission {
            self.permission.clone()
        }

        fn platform_name(&self) -> &'static str {
            "mock"
        }
    }

    fn engine(mock: Mock) -> Engine<Mock> {
        Engine::new(mock, Config::default())
    }

    fn applied_frame(engine: &Engine<Mock>) -> Rect {
        *engine
            .system()
            .applied
            .borrow()
            .last()
            .expect("no frame applied")
    }

    #[test]
    fn move_actions_change_position_only() {
        // (action, expected x, expected y) on a 1920x1040 work area with an
        // 800x600 window.
        let cases: [(Action, f64, f64); 9] = [
            (Action::TopLeft, 0.0, 0.0),
            (Action::TopRight, 1120.0, 0.0),
            (Action::BottomLeft, 0.0, 440.0),
            (Action::BottomRight, 1120.0, 440.0),
            (Action::Left, 0.0, 220.0),
            (Action::Right, 1120.0, 220.0),
            (Action::Top, 560.0, 0.0),
            (Action::Bottom, 560.0, 440.0),
            (Action::Center, 560.0, 220.0),
        ];

        for (action, x, y) in cases {
            let mut engine = engine(Mock::primary_only());
            engine.perform(action).unwrap();
            let frame = applied_frame(&engine);
            assert_eq!((frame.x, frame.y), (x, y), "{action}");
            assert_eq!(
                (frame.width, frame.height),
                (800.0, 600.0),
                "{action} must not resize the window"
            );
        }
    }

    #[test]
    fn move_actions_respect_the_margin_without_resizing() {
        let mut config = Config::default();
        config.set_margin(10.0);
        let mut engine = Engine::new(Mock::primary_only(), config);

        engine.perform(Action::TopLeft).unwrap();
        let frame = applied_frame(&engine);
        assert_eq!((frame.x, frame.y), (10.0, 10.0));
        assert_eq!((frame.width, frame.height), (800.0, 600.0));
    }

    #[test]
    fn move_actions_never_shrink_an_oversized_window() {
        let mock = Mock::with_displays(
            vec![display(1, 0.0, 0.0, 1920.0, 1040.0, true)],
            Rect::new(-100.0, -50.0, 2400.0, 1300.0),
        );
        let mut engine = engine(mock);
        engine.perform(Action::Center).unwrap();
        let frame = applied_frame(&engine);
        assert_eq!((frame.width, frame.height), (2400.0, 1300.0));
    }

    #[test]
    fn tile_actions_resize_to_quarters_and_halves() {
        let cases = [
            (Action::TileTopLeft, Rect::new(0.0, 0.0, 960.0, 520.0)),
            (Action::TileTopRight, Rect::new(960.0, 0.0, 960.0, 520.0)),
            (Action::TileBottomLeft, Rect::new(0.0, 520.0, 960.0, 520.0)),
            (
                Action::TileBottomRight,
                Rect::new(960.0, 520.0, 960.0, 520.0),
            ),
            (Action::TileLeftHalf, Rect::new(0.0, 0.0, 960.0, 1040.0)),
            (Action::TileRightHalf, Rect::new(960.0, 0.0, 960.0, 1040.0)),
            (Action::TileTopHalf, Rect::new(0.0, 0.0, 1920.0, 520.0)),
            (Action::TileBottomHalf, Rect::new(0.0, 520.0, 1920.0, 520.0)),
        ];
        for (action, expected) in cases {
            let mut engine = engine(Mock::primary_only());
            engine.perform(action).unwrap();
            assert_eq!(applied_frame(&engine), expected, "{action}");
        }
    }

    #[test]
    fn tile_actions_respect_the_margin() {
        let mut config = Config::default();
        config.set_margin(10.0);
        let mut engine = Engine::new(Mock::primary_only(), config);
        engine.perform(Action::TileTopLeft).unwrap();
        assert_eq!(applied_frame(&engine), Rect::new(10.0, 10.0, 940.0, 500.0));
    }

    #[test]
    fn maximize_fills_the_work_area() {
        let mut engine = engine(Mock::primary_only());
        engine.perform(Action::Maximize).unwrap();
        assert_eq!(applied_frame(&engine), Rect::new(0.0, 0.0, 1920.0, 1040.0));
    }

    #[test]
    fn repeated_action_is_a_no_op() {
        let mut engine = engine(Mock::primary_only());
        engine.perform(Action::Left).unwrap();
        let outcome = engine.perform(Action::Left).unwrap();
        assert!(matches!(outcome, Outcome::NoOp { .. }), "{outcome:?}");
        assert_eq!(engine.system().apply_count(), 1);
    }

    #[test]
    fn no_focused_window_is_reported() {
        let mut mock = Mock::primary_only();
        mock.focused = None;
        let mut engine = engine(mock);
        let err = engine.perform(Action::Left).unwrap_err();
        assert_eq!(err, EngineError::Platform(PlatformError::NoFocusedWindow));
        assert!(err.to_string().contains("没有找到"));
    }

    #[test]
    fn minimize_sets_the_platform_flag() {
        let mut engine = engine(Mock::primary_only());
        let outcome = engine.perform(Action::Minimize).unwrap();
        assert_eq!(
            outcome,
            Outcome::Applied {
                action: Action::Minimize,
                window: WindowId(42),
                frame: None,
            }
        );
        assert!(*engine.system().minimized.borrow());
        assert_eq!(engine.system().apply_count(), 0);
    }

    #[test]
    fn maximize_then_restore_round_trips() {
        let original = Rect::new(300.0, 200.0, 800.0, 600.0);
        let mut engine = engine(Mock::primary_only());
        engine.perform(Action::Maximize).unwrap();
        assert_eq!(applied_frame(&engine), Rect::new(0.0, 0.0, 1920.0, 1040.0));

        engine.perform(Action::Restore).unwrap();
        assert_eq!(applied_frame(&engine), original);
    }

    #[test]
    fn restore_is_single_use() {
        let mut engine = engine(Mock::primary_only());
        engine.perform(Action::TopLeft).unwrap();
        engine.perform(Action::Restore).unwrap();
        let outcome = engine.perform(Action::Restore).unwrap();
        assert!(matches!(outcome, Outcome::NoOp { .. }), "{outcome:?}");
        assert_eq!(engine.system().apply_count(), 2);
    }

    #[test]
    fn restore_after_move_returns_to_the_previous_position() {
        let mut engine = engine(Mock::primary_only());
        engine.perform(Action::TopLeft).unwrap();
        let after_first = engine.system().last_frame();
        engine.perform(Action::BottomRight).unwrap();
        engine.perform(Action::Restore).unwrap();
        assert_eq!(engine.system().last_frame(), after_first);
    }

    #[test]
    fn restore_falls_back_to_shrinking_an_os_maximized_window() {
        // No frame was remembered: the window already fills the work area.
        let mock = Mock::primary_only();
        *mock.frame.borrow_mut() = Rect::new(0.0, 0.0, 1920.0, 1040.0);
        let mut engine = engine(mock);

        let outcome = engine.perform(Action::Restore).unwrap();
        assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
        let frame = applied_frame(&engine);
        assert_eq!((frame.width, frame.height), (1536.0, 832.0));
        assert_eq!(frame.center(), Rect::new(0.0, 0.0, 1920.0, 1040.0).center());
    }

    #[test]
    fn restore_without_history_is_a_no_op() {
        let mut engine = engine(Mock::primary_only());
        let outcome = engine.perform(Action::Restore).unwrap();
        assert!(matches!(outcome, Outcome::NoOp { .. }), "{outcome:?}");
        assert_eq!(engine.system().apply_count(), 0);
    }

    #[test]
    fn moving_to_next_display_keeps_the_size() {
        let mut engine = engine(Mock::two_displays());
        engine.perform(Action::NextDisplay).unwrap();
        let frame = applied_frame(&engine);
        let target = Rect::new(1920.0, 0.0, 1280.0, 720.0);
        assert_eq!((frame.width, frame.height), (800.0, 600.0));
        assert!(target.contains_point(frame.center()), "{frame}");
    }

    #[test]
    fn moving_cycles_through_displays() {
        let mut engine = engine(Mock::two_displays());
        engine.perform(Action::NextDisplay).unwrap();
        let on_second = engine.system().last_frame();
        engine.perform(Action::NextDisplay).unwrap();
        let back_on_first = engine.system().last_frame();
        assert!(back_on_first.x < 1920.0);
        assert!(
            Rect::new(0.0, 0.0, 1920.0, 1040.0).contains_point(back_on_first.center()),
            "{back_on_first}"
        );
        assert_ne!(on_second, back_on_first);
    }

    #[test]
    fn previous_display_wraps_backwards() {
        let mut engine = engine(Mock::two_displays());
        engine.perform(Action::PreviousDisplay).unwrap();
        let frame = applied_frame(&engine);
        assert!(frame.x >= 1920.0, "{frame}");
        assert_eq!((frame.width, frame.height), (800.0, 600.0));
    }

    #[test]
    fn single_display_move_is_a_no_op() {
        let mut engine = engine(Mock::primary_only());
        let outcome = engine.perform(Action::NextDisplay).unwrap();
        assert!(matches!(outcome, Outcome::NoOp { .. }), "{outcome:?}");
        assert_eq!(engine.system().apply_count(), 0);
    }

    #[test]
    fn status_messages_mention_frames() {
        let mut engine = engine(Mock::primary_only());
        let outcome = engine.perform(Action::Left).unwrap();
        let message = outcome.status_message(Action::Left.label());
        assert!(message.contains("靠左"));
        assert!(message.contains("800x600"));
    }

    #[test]
    fn permission_and_platform_are_exposed() {
        let mut mock = Mock::primary_only();
        mock.permission = Permission::Missing("辅助功能权限".into());
        let engine = engine(mock);
        assert!(!engine.permission().is_granted());
        assert_eq!(engine.platform_name(), "mock");
    }
}
