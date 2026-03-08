//! A draggable, reorderable tab bar widget for Iced.
//!
//! Provides Chrome-style drag-and-drop tab reordering with visual feedback:
//! ghost tab follows cursor, source tab dims, tabs reorder live as cursor
//! crosses midpoints.
//!
//! # Usage
//!
//! ```rust,no_run
//! use iced_draggable_tabs::DraggableTabs;
//!
//! // In your view function:
//! let tabs = DraggableTabs::new(
//!     &["General", "Settings", "About"],
//!     active_tab_index,
//!     |idx| Message::TabSelected(idx),
//!     |new_order| Message::TabsReordered(new_order),
//! );
//! ```

use iced::advanced::layout::{self, Layout, Node};
use iced::advanced::overlay;
use iced::advanced::renderer;
use iced::advanced::text;
use iced::advanced::widget::{self, Tree, Widget};
use iced::advanced::{Clipboard, Shell};
use iced::mouse;
use iced::{
    Background, Border, Color, Element, Event, Length, Padding, Point,
    Rectangle, Size, Theme,
};

/// Configuration for the tab bar appearance.
#[derive(Debug, Clone)]
pub struct TabStyle {
    /// Background color for inactive tabs.
    pub inactive_background: Option<Color>,
    /// Background color for the active tab.
    pub active_background: Option<Color>,
    /// Text color for inactive tabs.
    pub inactive_text_color: Option<Color>,
    /// Text color for the active tab.
    pub active_text_color: Option<Color>,
    /// Border settings for tabs.
    pub border: Border,
    /// Tab height in pixels.
    pub tab_height: f32,
    /// Horizontal padding inside each tab.
    pub tab_padding: Padding,
    /// Gap between tabs.
    pub spacing: f32,
    /// Text size for tab labels.
    pub text_size: f32,
    /// Ghost tab opacity during drag (0.0–1.0).
    pub ghost_opacity: f32,
    /// Dimmed text opacity for the source tab during drag.
    pub dim_opacity: f32,
    /// Pixel distance before a click becomes a drag.
    pub drag_threshold: f32,
    /// Background color for the tab bar itself.
    pub bar_background: Option<Color>,
    /// Whether to show a close button on tabs.
    pub closeable: bool,
}

impl Default for TabStyle {
    fn default() -> Self {
        Self {
            inactive_background: None,
            active_background: None,
            inactive_text_color: None,
            active_text_color: None,
            border: Border {
                color: Color::TRANSPARENT,
                width: 0.0,
                radius: 6.0.into(),
            },
            tab_height: 36.0,
            tab_padding: Padding::from([8.0, 16.0]),
            spacing: 2.0,
            text_size: 14.0,
            ghost_opacity: 0.8,
            dim_opacity: 0.35,
            drag_threshold: 8.0,
            bar_background: None,
            closeable: false,
        }
    }
}

/// Internal drag state, stored in the widget tree.
#[derive(Debug, Default)]
struct DragState {
    /// Index of tab being dragged (in current order).
    dragging: Option<usize>,
    /// Mouse position at press start (in content coordinate space).
    press_start: Option<Point>,
    /// Whether drag threshold has been exceeded.
    is_active_drag: bool,
    /// Current cursor position during drag (content space, for reorder logic).
    cursor_position: Point,
    /// Raw cursor position from CursorMoved event (window space, for overlay).
    cursor_position_window: Point,
    /// Widget's screen-space Y, computed as raw_cursor.y - relative_cursor.y.
    widget_screen_y: f32,
    /// Per-tab widths based on text measurement.
    tab_widths: Vec<f32>,
    /// Tab bounds cache (recalculated each layout, in content coordinate space).
    tab_bounds: Vec<Rectangle>,
    /// Current tab order — indices into the original labels array.
    /// Starts as [0, 1, 2, ...] and gets reordered during drag.
    order: Vec<usize>,
    /// Whether the order was changed during this drag.
    moved: bool,
    /// Last known widget bounds.
    widget_bounds: Rectangle,
}

/// A draggable, reorderable tab bar widget.
///
/// Renders a horizontal row of tab buttons that can be reordered via
/// drag-and-drop. Emits messages when a tab is selected or when the
/// order changes.
pub struct DraggableTabs<'a, Message> {
    labels: &'a [&'a str],
    active: usize,
    on_select: Box<dyn Fn(usize) -> Message + 'a>,
    on_reorder: Box<dyn Fn(Vec<usize>) -> Message + 'a>,
    on_close: Option<Box<dyn Fn(usize) -> Message + 'a>>,
    style: TabStyle,
    width: Length,
}

impl<'a, Message> DraggableTabs<'a, Message> {
    /// Create a new draggable tab bar.
    ///
    /// - `labels`: slice of tab label strings
    /// - `active`: index of the currently active tab
    /// - `on_select`: called with the tab index when clicked
    /// - `on_reorder`: called with the new order (Vec of original indices) after drag
    pub fn new(
        labels: &'a [&'a str],
        active: usize,
        on_select: impl Fn(usize) -> Message + 'a,
        on_reorder: impl Fn(Vec<usize>) -> Message + 'a,
    ) -> Self {
        Self {
            labels,
            active,
            on_select: Box::new(on_select),
            on_reorder: Box::new(on_reorder),
            on_close: None,
            style: TabStyle::default(),
            width: Length::Fill,
        }
    }

    /// Set the tab bar width.
    pub fn width(mut self, width: Length) -> Self {
        self.width = width;
        self
    }

    /// Set the tab height.
    pub fn tab_height(mut self, height: f32) -> Self {
        self.style.tab_height = height;
        self
    }

    /// Set spacing between tabs.
    pub fn spacing(mut self, spacing: f32) -> Self {
        self.style.spacing = spacing;
        self
    }

    /// Set the drag threshold in pixels.
    pub fn drag_threshold(mut self, threshold: f32) -> Self {
        self.style.drag_threshold = threshold;
        self
    }

    /// Set the ghost tab opacity during drag.
    pub fn ghost_opacity(mut self, opacity: f32) -> Self {
        self.style.ghost_opacity = opacity;
        self
    }

    /// Set the text size for tab labels.
    pub fn text_size(mut self, size: f32) -> Self {
        self.style.text_size = size;
        self
    }

    /// Set tab padding.
    pub fn tab_padding(mut self, padding: impl Into<Padding>) -> Self {
        self.style.tab_padding = padding.into();
        self
    }

    /// Set the border for tabs.
    pub fn tab_border(mut self, border: Border) -> Self {
        self.style.border = border;
        self
    }

    /// Set inactive tab background color.
    pub fn inactive_background(mut self, color: Color) -> Self {
        self.style.inactive_background = Some(color);
        self
    }

    /// Set active tab background color.
    pub fn active_background(mut self, color: Color) -> Self {
        self.style.active_background = Some(color);
        self
    }

    /// Set inactive tab text color.
    pub fn inactive_text_color(mut self, color: Color) -> Self {
        self.style.inactive_text_color = Some(color);
        self
    }

    /// Set active tab text color.
    pub fn active_text_color(mut self, color: Color) -> Self {
        self.style.active_text_color = Some(color);
        self
    }

    /// Set the tab bar background color.
    pub fn bar_background(mut self, color: Color) -> Self {
        self.style.bar_background = Some(color);
        self
    }

    /// Enable close buttons on tabs with a callback.
    pub fn on_close(mut self, on_close: impl Fn(usize) -> Message + 'a) -> Self {
        self.on_close = Some(Box::new(on_close));
        self.style.closeable = true;
        self
    }

    /// Apply a complete style configuration.
    pub fn style(mut self, style: TabStyle) -> Self {
        self.style = style;
        self
    }
}

impl<'a, Message: Clone> Widget<Message, Theme, iced::Renderer>
    for DraggableTabs<'a, Message>
{
    fn tag(&self) -> widget::tree::Tag {
        widget::tree::Tag::of::<DragState>()
    }

    fn state(&self) -> widget::tree::State {
        widget::tree::State::new(DragState::default())
    }

    fn size(&self) -> Size<Length> {
        Size {
            width: self.width,
            height: Length::Shrink,
        }
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        _renderer: &iced::Renderer,
        limits: &layout::Limits,
    ) -> Node {
        let limits = limits.width(self.width);
        let max_width = limits.max().width;
        let height = self.style.tab_height;
        let n = self.labels.len();

        if n == 0 {
            return Node::new(Size::new(0.0, height));
        }

        // Measure each tab's text width using the renderer
        let h_pad = self.style.tab_padding.left + self.style.tab_padding.right;
        let close_extra = if self.style.closeable { 20.0 } else { 0.0 };

        let state = tree.state.downcast_mut::<DragState>();
        state.tab_widths.clear();

        for label in self.labels.iter() {
            let paragraph: text::paragraph::Plain<<iced::Renderer as text::Renderer>::Paragraph> =
                text::paragraph::Plain::new(text::Text {
                    content: label.to_string(),
                    bounds: Size::new(f32::INFINITY, height),
                    size: self.style.text_size.into(),
                    line_height: iced::widget::text::LineHeight::default(),
                    font: iced::Font::default(),
                    align_x: iced::alignment::Horizontal::Center.into(),
                    align_y: iced::alignment::Vertical::Center.into(),
                    shaping: text::Shaping::Advanced,
                    wrapping: text::Wrapping::None,
                });
            let text_width = paragraph.min_bounds().width;
            let tab_w = (text_width + h_pad + close_extra).max(40.0);
            state.tab_widths.push(tab_w);
        }

        let total_spacing = self.style.spacing * (n as f32 - 1.0).max(0.0);
        let natural_width: f32 = state.tab_widths.iter().sum::<f32>() + total_spacing;

        // If tabs exceed available width, scale them down proportionally
        if natural_width > max_width && natural_width > 0.0 {
            let scale = (max_width - total_spacing) / (natural_width - total_spacing);
            for w in state.tab_widths.iter_mut() {
                *w = (*w * scale).max(40.0);
            }
        }

        let final_width: f32 = state.tab_widths.iter().sum::<f32>() + total_spacing;
        Node::new(Size::new(final_width.min(max_width), height))
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _renderer: &iced::Renderer,
        _clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_mut::<DragState>();
        let bounds = layout.bounds();
        state.widget_bounds = bounds;

        // Initialize order if empty or size changed
        if state.order.len() != self.labels.len() {
            state.order = (0..self.labels.len()).collect();
        }

        // Recalculate tab bounds using per-tab widths from layout
        let n = self.labels.len();
        state.tab_bounds.clear();
        let mut x = bounds.x;
        for i in 0..n {
            let w = state.tab_widths.get(i).copied().unwrap_or(60.0);
            state.tab_bounds.push(Rectangle {
                x,
                y: bounds.y,
                width: w,
                height: bounds.height,
            });
            x += w + self.style.spacing;
        }

        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                if let Some(pos) = cursor.position_in(bounds) {
                    let abs = Point::new(bounds.x + pos.x, bounds.y + pos.y);

                    // Find which tab was pressed (in display order)
                    for (display_idx, tab_rect) in state.tab_bounds.iter().enumerate() {
                        if tab_rect.contains(abs) {
                            // Check close button hit
                            if self.style.closeable {
                                let close_x = tab_rect.x + tab_rect.width - 20.0;
                                let close_y = tab_rect.y + (tab_rect.height - 14.0) / 2.0;
                                let close_rect = Rectangle {
                                    x: close_x,
                                    y: close_y,
                                    width: 14.0,
                                    height: 14.0,
                                };
                                if close_rect.contains(abs) {
                                    if let Some(ref on_close) = self.on_close {
                                        let original_idx = state.order[display_idx];
                                        shell.publish((on_close)(original_idx));
                                        shell.capture_event();
                                        return;
                                    }
                                }
                            }

                            state.dragging = Some(display_idx);
                            state.press_start = Some(abs);
                            state.is_active_drag = false;
                            state.moved = false;
                            shell.request_redraw();
                            shell.capture_event();
                            return;
                        }
                    }
                }
            }

            Event::Mouse(mouse::Event::CursorMoved { position: raw_pos }) => {
                // raw_pos = window space (for overlay ghost positioning)
                // cursor.position() = content space (same as layout.bounds(), for reorder)
                let raw_position = *raw_pos;
                state.cursor_position_window = raw_position;

                if let Some(position) = cursor.position() {
                    state.cursor_position = position;

                    // Compute widget's screen Y from the difference between
                    // raw window coords and relative-to-widget coords
                    if let Some(rel) = cursor.position_in(bounds) {
                        state.widget_screen_y = raw_position.y - rel.y;
                    }

                    if let (Some(drag_idx), Some(start)) =
                        (state.dragging, state.press_start)
                    {
                        if !state.is_active_drag {
                            // Check threshold
                            let dx = (position.x - start.x).abs();
                            let dy = (position.y - start.y).abs();
                            if dx.max(dy) > self.style.drag_threshold {
                                state.is_active_drag = true;
                            }
                            shell.request_redraw();
                            shell.capture_event();
                            return;
                        }

                        // Live reorder: check if cursor has crossed a tab midpoint
                        // Only check X position — tabs are horizontal
                        if state.is_active_drag {
                            for (i, tab_rect) in state.tab_bounds.iter().enumerate() {
                                if i == drag_idx {
                                    continue;
                                }
                                let mid_x = tab_rect.x + tab_rect.width / 2.0;

                                let should_swap = if i < drag_idx {
                                    position.x < mid_x
                                } else {
                                    position.x > mid_x
                                };

                                if should_swap {
                                    let item = state.order.remove(drag_idx);
                                    state.order.insert(i, item);
                                    state.dragging = Some(i);
                                    state.moved = true;
                                    break;
                                }
                            }
                        }
                        shell.request_redraw();
                        shell.capture_event();
                        return;
                    }
                }
            }

            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                if let Some(drag_idx) = state.dragging.take() {
                    let was_drag = state.is_active_drag;
                    let moved = state.moved;
                    state.is_active_drag = false;
                    state.press_start = None;
                    state.moved = false;

                    if was_drag && moved {
                        shell.publish((self.on_reorder)(state.order.clone()));
                    } else if !was_drag {
                        let original_idx = state.order[drag_idx];
                        shell.publish((self.on_select)(original_idx));
                    }
                    shell.request_redraw();
                    shell.capture_event();
                    return;
                }
            }

            _ => {}
        }
    }

    fn draw(
        &self,
        tree: &widget::Tree,
        renderer: &mut iced::Renderer,
        theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_ref::<DragState>();
        let bounds = layout.bounds();
        let palette = theme.palette();
        let extended = theme.extended_palette();

        // Draw bar background
        if let Some(bg) = self.style.bar_background {
            renderer::Renderer::fill_quad(
                renderer,
                renderer::Quad {
                    bounds,
                    border: Border::default(),
                    shadow: iced::Shadow::default(),
                    snap: false,
                },
                Background::Color(bg),
            );
        }

        let n = self.labels.len();
        if n == 0 {
            return;
        }

        // Default colors from theme
        let active_bg = self
            .style
            .active_background
            .unwrap_or(extended.primary.base.color);
        let inactive_bg = self
            .style
            .inactive_background
            .unwrap_or(extended.background.strong.color);
        let active_text = self
            .style
            .active_text_color
            .unwrap_or_else(|| auto_contrast(active_bg));
        let inactive_text = self
            .style
            .inactive_text_color
            .unwrap_or(palette.text);

        // Draw each tab using per-tab widths
        let mut x = bounds.x;
        for (display_idx, &original_idx) in state.order.iter().enumerate() {
            if original_idx >= self.labels.len() {
                continue;
            }

            let tab_width = state.tab_widths.get(original_idx).copied().unwrap_or(60.0);
            let tab_rect = Rectangle {
                x,
                y: bounds.y,
                width: tab_width,
                height: bounds.height,
            };
            x += tab_width + self.style.spacing;

            let is_active = original_idx == self.active;
            let is_being_dragged =
                state.is_active_drag && state.dragging == Some(display_idx);

            // Tab background
            let bg_color = if is_active { active_bg } else { inactive_bg };

            if is_being_dragged {
                // Placeholder: dotted border, very faint background
                renderer::Renderer::fill_quad(
                    renderer,
                    renderer::Quad {
                        bounds: tab_rect,
                        border: Border {
                            color: Color { a: 0.4, ..active_bg },
                            width: 2.0,
                            ..self.style.border
                        },
                        shadow: iced::Shadow::default(),
                        snap: false,
                    },
                    Background::Color(Color {
                        a: 0.15,
                        ..bg_color
                    }),
                );
            } else {
                renderer::Renderer::fill_quad(
                    renderer,
                    renderer::Quad {
                        bounds: tab_rect,
                        border: self.style.border,
                        shadow: iced::Shadow::default(),
                        snap: false,
                    },
                    Background::Color(bg_color),
                );
            }

            // Tab label
            let text_color = if is_active { active_text } else { inactive_text };
            let label = self.labels[original_idx];

            let text_alpha = if is_being_dragged {
                0.2
            } else {
                1.0
            };

            // Close button takes 20px from the right if enabled
            let text_max_width = if self.style.closeable {
                tab_width - self.style.tab_padding.left - self.style.tab_padding.right - 20.0
            } else {
                tab_width - self.style.tab_padding.left - self.style.tab_padding.right
            };

            let text_bounds = Rectangle {
                x: tab_rect.x + self.style.tab_padding.left,
                y: tab_rect.y,
                width: text_max_width.max(0.0),
                height: tab_rect.height,
            };

            text::Renderer::fill_text(
                renderer,
                text::Text {
                    content: label.to_string(),
                    bounds: Size::new(text_bounds.width, text_bounds.height),
                    size: self.style.text_size.into(),
                    line_height: iced::widget::text::LineHeight::default(),
                    font: iced::Font::default(),
                    align_x: iced::alignment::Horizontal::Center.into(),
                    align_y: iced::alignment::Vertical::Center.into(),
                    shaping: text::Shaping::Advanced,
                    wrapping: text::Wrapping::None,
                },
                Point::new(
                    text_bounds.x + text_bounds.width / 2.0,
                    text_bounds.y + text_bounds.height / 2.0,
                ),
                Color {
                    a: text_alpha,
                    ..text_color
                },
                text_bounds,
            );

            // Draw close button if enabled
            if self.style.closeable {
                let close_x = tab_rect.x + tab_rect.width - 18.0;
                let close_y = tab_rect.y + (tab_rect.height - 14.0) / 2.0;

                text::Renderer::fill_text(
                    renderer,
                    text::Text {
                        content: "\u{00D7}".to_string(), // × symbol
                        bounds: Size::new(14.0, 14.0),
                        size: 14.0.into(),
                        line_height: iced::widget::text::LineHeight::default(),
                        font: iced::Font::default(),
                        align_x: iced::alignment::Horizontal::Center.into(),
                        align_y: iced::alignment::Vertical::Center.into(),
                        shaping: text::Shaping::Basic,
                        wrapping: text::Wrapping::None,
                    },
                    Point::new(close_x + 7.0, close_y + 7.0),
                    Color {
                        a: 0.6,
                        ..text_color
                    },
                    Rectangle {
                        x: close_x,
                        y: close_y,
                        width: 14.0,
                        height: 14.0,
                    },
                );
            }
        }

    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'_>,
        _renderer: &iced::Renderer,
        _viewport: &Rectangle,
        _translation: iced::Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, iced::Renderer>> {
        let state = tree.state.downcast_ref::<DragState>();
        if !state.is_active_drag {
            return None;
        }
        let drag_display_idx = state.dragging?;
        let &original_idx = state.order.get(drag_display_idx)?;
        if original_idx >= self.labels.len() {
            return None;
        }

        let bounds = layout.bounds();
        let tab_width = state.tab_widths.get(original_idx).copied().unwrap_or(60.0);

        // The overlay renders in window space. Use window-space coordinates
        // computed during update: raw cursor X for horizontal follow,
        // and widget_screen_y for vertical anchoring near the tab bar.
        let ghost_x = state.cursor_position_window.x - tab_width / 2.0;
        let ghost_y = state.widget_screen_y - 4.0;

        Some(overlay::Element::new(Box::new(GhostOverlay {
            _phantom: std::marker::PhantomData,
            position: Point::new(ghost_x, ghost_y),
            size: Size::new(tab_width, bounds.height),
            label: self.labels[original_idx].to_string(),
            is_active: original_idx == self.active,
            style: self.style.clone(),
        })))
    }

    fn mouse_interaction(
        &self,
        tree: &widget::Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _viewport: &Rectangle,
        _renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        let state = tree.state.downcast_ref::<DragState>();

        if state.is_active_drag {
            return mouse::Interaction::Grabbing;
        }

        if cursor.position_in(layout.bounds()).is_some() {
            return mouse::Interaction::Pointer;
        }

        mouse::Interaction::default()
    }
}

/// Convert a `DraggableTabs` into an `Element`.
impl<'a, Message: Clone + 'a> From<DraggableTabs<'a, Message>>
    for Element<'a, Message>
{
    fn from(tabs: DraggableTabs<'a, Message>) -> Self {
        Self::new(tabs)
    }
}

/// Ghost tab overlay — renders the floating ghost above all other widgets.
struct GhostOverlay<Message> {
    _phantom: std::marker::PhantomData<Message>,
    position: Point,
    size: Size,
    label: String,
    is_active: bool,
    style: TabStyle,
}

impl<Message> overlay::Overlay<Message, Theme, iced::Renderer> for GhostOverlay<Message> {
    fn layout(&mut self, _renderer: &iced::Renderer, _bounds: Size) -> Node {
        Node::new(self.size).move_to(self.position)
    }

    fn draw(
        &self,
        renderer: &mut iced::Renderer,
        theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
    ) {
        let ghost_rect = layout.bounds();
        let extended = theme.extended_palette();
        let palette = theme.palette();

        let active_bg = self
            .style
            .active_background
            .unwrap_or(extended.primary.base.color);
        let inactive_bg = self
            .style
            .inactive_background
            .unwrap_or(extended.background.strong.color);
        let bg_color = if self.is_active { active_bg } else { inactive_bg };
        let text_color = if self.is_active {
            self.style
                .active_text_color
                .unwrap_or_else(|| auto_contrast(active_bg))
        } else {
            self.style.inactive_text_color.unwrap_or(palette.text)
        };

        // Ghost background with prominent border and shadow
        renderer::Renderer::fill_quad(
            renderer,
            renderer::Quad {
                bounds: ghost_rect,
                border: Border {
                    color: active_bg,
                    width: 2.0,
                    ..self.style.border
                },
                shadow: iced::Shadow {
                    color: Color::from_rgba(0.0, 0.0, 0.0, 0.4),
                    offset: iced::Vector::new(0.0, 3.0),
                    blur_radius: 10.0,
                },
                snap: false,
            },
            Background::Color(Color {
                a: self.style.ghost_opacity,
                ..bg_color
            }),
        );

        // Ghost label
        let text_bounds = Rectangle {
            x: ghost_rect.x + self.style.tab_padding.left,
            y: ghost_rect.y,
            width: (ghost_rect.width - self.style.tab_padding.left - self.style.tab_padding.right)
                .max(0.0),
            height: ghost_rect.height,
        };

        text::Renderer::fill_text(
            renderer,
            text::Text {
                content: self.label.clone(),
                bounds: Size::new(text_bounds.width, text_bounds.height),
                size: self.style.text_size.into(),
                line_height: iced::widget::text::LineHeight::default(),
                font: iced::Font::default(),
                align_x: iced::alignment::Horizontal::Center.into(),
                align_y: iced::alignment::Vertical::Center.into(),
                shaping: text::Shaping::Advanced,
                wrapping: text::Wrapping::None,
            },
            Point::new(
                text_bounds.x + text_bounds.width / 2.0,
                text_bounds.y + text_bounds.height / 2.0,
            ),
            Color {
                a: self.style.ghost_opacity,
                ..text_color
            },
            text_bounds,
        );
    }

}

/// Auto-contrast: returns black or white depending on background luminance.
fn auto_contrast(bg: Color) -> Color {
    let luminance = 0.299 * bg.r + 0.587 * bg.g + 0.114 * bg.b;
    if luminance > 0.5 {
        Color::BLACK
    } else {
        Color::WHITE
    }
}
