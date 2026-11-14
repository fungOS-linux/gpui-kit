//! A viewport for a GPU texture.
//!
//! The texture is sampled where it already lives: nothing is read back to the
//! CPU and nothing is uploaded again. That is the difference between showing a
//! render and blitting its pixels into a rectangle.
//!
//! The component knows the handle and nothing else — a `DynamicTexture` carries
//! its own size. It paints with the window's painter order and clipping, and it
//! forwards a press as a point in texture pixels, so the owner of the texture
//! decides what the press means.

use std::{cell::Cell, rc::Rc};

use gpui::{
    AnyElement, App, Bounds, DynamicTexture, ElementId, IntoElement, MouseButton, Pixels, Point,
    Window, canvas, div, point, prelude::*, px,
};

/// What a press on the texture is reported as: a point in texture pixels.
type ClickHandler = Box<dyn Fn(Point<Pixels>, &mut Window, &mut App)>;

/// Shows a GPU texture, stretched over the box it is given.
pub struct TextureView {
    id: ElementId,
    texture: DynamicTexture,
    on_click: Option<ClickHandler>,
}

impl TextureView {
    pub fn new(id: impl Into<ElementId>, texture: DynamicTexture) -> Self {
        Self {
            id: id.into(),
            texture,
            on_click: None,
        }
    }

    /// Forward a press as a point in texture pixels: top-left is (0, 0),
    /// bottom-right is the texture's own size. A press outside the painted
    /// rectangle is dropped.
    pub fn on_click(
        mut self,
        handler: impl Fn(Point<Pixels>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Box::new(handler));
        self
    }
}

impl IntoElement for TextureView {
    type Element = AnyElement;

    fn into_element(self) -> AnyElement {
        let Self {
            id,
            texture,
            on_click,
        } = self;
        let extent = texture.size();
        let size = (
            extent.width.0.max(1) as f32,
            extent.height.0.max(1) as f32,
        );
        // Where the texture actually landed last frame. A press is resolved
        // against the pixels a person saw, not against a re-derived rectangle.
        let painted: Rc<Cell<Option<Bounds<Pixels>>>> = Rc::new(Cell::new(None));

        let remember = painted.clone();
        let painter_id = id.clone();
        // `canvas` sizes to its content — which is nothing — so it is given the
        // whole box. Without this it lays out zero-height and paints air.
        let mut view = div().id(id).relative().size_full().child(
            canvas(
                move |bounds, _window, _cx| bounds,
                move |bounds, _, window, _cx| {
                    remember.set(Some(bounds));
                    if let Err(error) = window.paint_dynamic_texture(bounds, texture) {
                        // The texture outlives this frame's draw; a lost device
                        // is the host's problem to report, not ours to swallow.
                        eprintln!("texture view {:?}: {error:?}", painter_id);
                    }
                },
            )
            .size_full(),
        );

        if let Some(handler) = on_click {
            let painted = painted.clone();
            view = view.on_mouse_down(MouseButton::Left, move |event, window, cx| {
                let Some(dest) = painted.get() else {
                    return;
                };
                let width = f32::from(dest.size.width);
                let height = f32::from(dest.size.height);
                if width <= 0.0 || height <= 0.0 {
                    return;
                }
                let u = f32::from(event.position.x - dest.origin.x) / width;
                let v = f32::from(event.position.y - dest.origin.y) / height;
                if (0.0..=1.0).contains(&u) && (0.0..=1.0).contains(&v) {
                    handler(
                        point(px(u * size.0), px(v * size.1)),
                        window,
                        cx,
                    );
                }
            });
        }

        view.into_any_element()
    }
}
