use std::ops::RangeInclusive;

use iced::window::RedrawRequest;
use iced::{window, Element, Length};

impl StyleSheet for iced::Theme {
    type Style = ();

    fn appearance(&self, _style: &Self::Style) -> Appearance {
        let palette = self.extended_palette();

        Appearance {
            background: None,
            track_color: palette.background.weak.color,
            bar_color: palette.primary.base.color,
        }
    }
}

// stolen from the iced examples
// Show a ProgressCircle progress indicator.
use iced::advanced::layout;
use iced::advanced::renderer;
use iced::advanced::widget::tree::{self, Tree};
use iced::advanced::{self, Clipboard, Layout, Shell, Widget};
use iced::event;
use iced::mouse;
use iced::widget::canvas;
use iced::{Background, Color, Event, Radians, Rectangle, Renderer, Size, Vector};

use std::f32::consts::PI;

const MIN_ANGLE: Radians = Radians(0.0);
const ANGLE_OFFSET: Radians = Radians(PI / 2.0);
const WRAP_ANGLE: Radians = Radians(2.0 * PI - PI / 4.0);

#[allow(missing_debug_implementations)]
pub struct ProgressCircle<Theme>
where
    Theme: StyleSheet,
{
    size: f32,
    bar_height: f32,
    style: <Theme as StyleSheet>::Style,
    range: RangeInclusive<f32>,
    value: f32,
}

impl<Theme> ProgressCircle<Theme>
where
    Theme: StyleSheet,
{
    /// Creates a new [`ProgressCircle`] with the given content.
    pub fn new(range: RangeInclusive<f32>, value: f32) -> Self {
        ProgressCircle {
            size: 40.0,
            bar_height: 4.0,
            style: <Theme as StyleSheet>::Style::default(),
            value: value.clamp(*range.start(), *range.end()),
            range,
        }
    }

    /// Sets the size of the [`ProgressCircle`].
    pub fn size(mut self, size: f32) -> Self {
        self.size = size;
        self
    }

    /// Sets the bar height of the [`ProgressCircle`].
    pub fn bar_height(mut self, bar_height: f32) -> Self {
        self.bar_height = bar_height;
        self
    }

    /// Sets the style variant of this [`ProgressCircle`].
    pub fn style(mut self, style: <Theme as StyleSheet>::Style) -> Self {
        self.style = style;
        self
    }
}

struct State {
    cache: canvas::Cache,
    size: f32,
    bar_height: f32,
    range: RangeInclusive<f32>,
    value: f32,
}

impl<'a, Message, Theme> Widget<Message, Theme, Renderer> for ProgressCircle<Theme>
where
    Message: 'a + Clone,
    Theme: StyleSheet,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State {
            bar_height: self.bar_height,
            size: self.size,
            cache: canvas::Cache::default(),
            range: self.range.clone(),
            value: self.value,
        })
    }

    fn size(&self) -> Size<Length> {
        Size {
            width: Length::Fixed(self.size),
            height: Length::Fixed(self.size),
        }
    }

    fn layout(
        &self,
        _tree: &mut Tree,
        _renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        layout::atomic(limits, self.size, self.size)
    }

    fn on_event(
        &mut self,
        tree: &mut Tree,
        event: Event,
        _layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _renderer: &Renderer,
        _clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) -> event::Status {
        let state = tree.state.downcast_mut::<State>();

        if let Event::Window(window::Event::RedrawRequested(_)) = event {
            if state.size != self.size
                || state.bar_height != self.bar_height
                || state.range != self.range
                || state.value != self.value
            {
                state.cache.clear();
                state.range = self.range.clone();
                state.value = self.value;
                state.bar_height = self.bar_height;
                state.size = self.size;
            }

            state.cache.clear();
            shell.request_redraw(RedrawRequest::NextFrame);
        }

        event::Status::Ignored
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
        use advanced::graphics::geometry::Renderer as _;
        use advanced::Renderer as _;
        let state = tree.state.downcast_ref::<State>();
        let bounds = layout.bounds();
        let custom_style = <Theme as StyleSheet>::appearance(theme, &self.style);

        let geometry = state.cache.draw(renderer, bounds.size(), |frame| {
            let track_radius = frame.width() / 2.0 - self.bar_height;
            let track_path = canvas::Path::circle(frame.center(), track_radius);

            frame.stroke(
                &track_path,
                canvas::Stroke::default()
                    .with_color(custom_style.track_color)
                    .with_width(self.bar_height),
            );

            let mut builder = canvas::path::Builder::new();

            let start = ANGLE_OFFSET;

            let progress =
                (self.value - self.range.start()) / (self.range.end() - self.range.start());

            builder.arc(canvas::path::Arc {
                center: frame.center(),
                radius: track_radius,
                start_angle: start,
                end_angle: start + MIN_ANGLE + WRAP_ANGLE * progress,
            });

            let bar_path = builder.build();

            frame.stroke(
                &bar_path,
                canvas::Stroke::default()
                    .with_color(custom_style.bar_color)
                    .with_width(self.bar_height),
            );
        });

        renderer.with_translation(Vector::new(bounds.x, bounds.y), |renderer| {
            renderer.draw_geometry(geometry);
        });
    }
}

impl<'a, Message, Theme> From<ProgressCircle<Theme>> for Element<'a, Message, Theme, Renderer>
where
    Message: Clone + 'a,
    Theme: StyleSheet + 'a,
{
    fn from(progress_circle: ProgressCircle<Theme>) -> Self {
        Self::new(progress_circle)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Appearance {
    /// The [`Background`] of the progress indicator.
    pub background: Option<Background>,
    /// The track [`Color`] of the progress indicator.
    pub track_color: Color,
    /// The bar [`Color`] of the progress indicator.
    pub bar_color: Color,
}

impl std::default::Default for Appearance {
    fn default() -> Self {
        Self {
            background: None,
            track_color: Color::TRANSPARENT,
            bar_color: Color::BLACK,
        }
    }
}

/// A set of rules that dictate the style of an indicator.
pub trait StyleSheet {
    /// The supported style of the [`StyleSheet`].
    type Style: Default;

    /// Produces the active [`Appearance`] of a indicator.
    fn appearance(&self, style: &Self::Style) -> Appearance;
}
