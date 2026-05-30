use rio_backend::ansi::graphics::{KittyPlacement, StoredImage, VirtualPlacement};
use rio_backend::config::colors::term::TermColors;
use rio_backend::config::CursorConfig;
use rio_backend::crosswords::grid::row::Row;
use rio_backend::crosswords::pos::CursorState;
use rio_backend::crosswords::square::Square;
use rio_backend::event::TerminalDamage;
use rio_backend::selection::SelectionRange;
use rustc_hash::FxHashMap;
use std::time::Instant;

#[derive(Clone, Copy, Debug)]
pub enum BackgroundState {
    Set(rio_backend::sugarloaf::Color),
    Reset,
}

#[derive(Clone, Copy, Debug)]
pub enum WindowUpdate {
    Background(BackgroundState),
}

#[derive(Default, Clone, Debug)]
pub struct Cursor {
    pub state: CursorState,
    pub content: char,
    pub content_ref: char,
    pub is_ime_enabled: bool,
}

/// Hint label information for rendering.
#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct HintLabel {
    pub position: rio_backend::crosswords::pos::Pos,
    pub label: Vec<char>,
    pub is_first: bool,
}

#[derive(Default)]
pub struct RenderableContent {
    pub cursor: Cursor,
    pub has_blinking_enabled: bool,
    pub is_blinking_cursor_visible: bool,
    pub selection_range: Option<SelectionRange>,
    pub hyperlink_range: Option<SelectionRange>,
    pub hint_labels: Vec<HintLabel>,
    pub highlighted_hint: Option<crate::hints::HintMatch>,
    pub hint_matches: Option<Vec<rio_backend::crosswords::search::Match>>,
    pub last_typing: Option<Instant>,
    pub last_blink_toggle: Option<Instant>,
    pub pending_update: PendingUpdate,
    pub background: Option<BackgroundState>,
    /// Damage hint for the in-progress frame. Set by `Renderer::run`
    /// from PTY + UI damage merging, consumed by `Screen::render`'s
    /// grid emit to choose whether to rebuild no rows, dirty rows, or
    /// all rows.
    pub frame_damage: TerminalDamage,

    /// Per-context viewport row buffer reused across frames.
    pub visible_rows: Vec<Row<Square>>,
    /// Parallel per-cell resolved styles, flat row-major.
    pub cell_styles: Vec<rio_backend::crosswords::style::Style>,
    /// Snapshot of visible cell extras keyed by `extras_id`.
    pub extras: FxHashMap<u16, rio_backend::crosswords::square::Extras>,
    pub term_colors: TermColors,
    pub display_offset: usize,
    pub columns: usize,
    pub screen_lines: usize,
    pub history_size: usize,
    pub blinking_cursor: bool,
    pub kitty_virtual_placements: FxHashMap<(u32, u32), VirtualPlacement>,
    pub kitty_images: FxHashMap<u32, StoredImage>,
    pub kitty_placements: Vec<KittyPlacement>,
    pub kitty_graphics_dirty: bool,
}

impl RenderableContent {
    pub fn new(cursor: Cursor) -> Self {
        RenderableContent {
            cursor,
            has_blinking_enabled: false,
            selection_range: None,
            hint_labels: Vec::new(),
            highlighted_hint: None,
            hint_matches: None,
            last_typing: None,
            last_blink_toggle: None,
            hyperlink_range: None,
            pending_update: PendingUpdate::default(),
            is_blinking_cursor_visible: false,
            background: None,
            frame_damage: TerminalDamage::Full,
            visible_rows: Vec::new(),
            cell_styles: Vec::new(),
            extras: FxHashMap::default(),
            term_colors: TermColors::default(),
            display_offset: 0,
            columns: 0,
            screen_lines: 0,
            history_size: 0,
            blinking_cursor: false,
            kitty_virtual_placements: FxHashMap::default(),
            kitty_images: FxHashMap::default(),
            kitty_placements: Vec::new(),
            kitty_graphics_dirty: false,
        }
    }

    pub fn from_cursor_config(config_cursor: &CursorConfig) -> Self {
        let cursor = Cursor {
            content: config_cursor.shape.into(),
            content_ref: config_cursor.shape.into(),
            state: CursorState::new(config_cursor.shape.into()),
            is_ime_enabled: false,
        };
        Self::new(cursor)
    }
}

#[derive(Debug, Default)]
pub struct PendingUpdate {
    /// Whether there's any pending update that needs rendering.
    dirty: bool,
    /// Terminal content damage.
    terminal_damage: Option<TerminalDamage>,
}

impl PendingUpdate {
    #[inline]
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    #[inline]
    pub fn has_terminal_damage(&self) -> bool {
        self.terminal_damage.is_some()
    }

    /// Mark as needing to check for damage on next render.
    pub fn set_dirty(&mut self) {
        self.dirty = true;
    }

    pub fn set_terminal_damage(&mut self, damage: TerminalDamage) {
        self.dirty = true;
        self.terminal_damage = Some(match self.terminal_damage.take() {
            None => damage,
            Some(existing) => Self::merge_terminal_damages(existing, damage),
        });
    }

    pub fn take_terminal_damage(&mut self) -> Option<TerminalDamage> {
        self.terminal_damage.take()
    }

    pub fn reset(&mut self) {
        self.dirty = false;
    }

    /// Merge two terminal damage hints into one. Strict ordering by
    /// amount of work needed: Full > Partial > CursorOnly > Noop.
    pub fn merge_terminal_damages(
        existing: TerminalDamage,
        new: TerminalDamage,
    ) -> TerminalDamage {
        use TerminalDamage::*;
        match (existing, new) {
            (Full, _) | (_, Full) => Full,
            (Partial, _) | (_, Partial) => Partial,
            (CursorOnly, _) | (_, CursorOnly) => CursorOnly,
            (Noop, Noop) => Noop,
        }
    }
}
