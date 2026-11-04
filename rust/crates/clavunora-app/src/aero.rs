// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 redgion and Clavunora contributors
//! Summer Aero: native glass surfaces sample a blurred copy of the same wallpaper.
use anyhow::Result;
use eframe::egui::{
    self, Align2, Color32, FontId, Pos2, Rect, Response, Sense, Stroke, StrokeKind, TextureHandle,
    Ui, Vec2,
    epaint::{Mesh, Vertex},
};

#[derive(Clone, Copy)]
pub enum Icon {
    Library,
    Course,
    Note,
    Trophy,
    Chart,
    Settings,
    Folder,
    Star,
    Piano,
}
pub struct Aero {
    background: TextureHandle,
    blurred: TextureHandle,
    screen: Rect,
    uv: Rect,
    pub plain: bool,
}
pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "clavunora-brand".into(),
        egui::FontData::from_static(include_bytes!("../assets/brand-serif.ttf")).into(),
    );
    fonts.families.insert(
        egui::FontFamily::Name("brand".into()),
        vec!["clavunora-brand".into()],
    );
    ctx.set_fonts(fonts);
}
pub fn text(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgb(234, 250, 255)
    } else {
        Color32::from_rgb(12, 49, 68)
    }
}
pub fn muted(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgb(169, 210, 225)
    } else {
        Color32::from_rgb(42, 87, 107)
    }
}
pub fn style(ctx: &egui::Context, dark: bool) {
    let mut style = (*ctx.global_style()).clone();
    style.visuals = if dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
    style.visuals.override_text_color = Some(text(dark));
    style.visuals.panel_fill = Color32::TRANSPARENT;
    style.visuals.window_fill = if dark {
        Color32::from_rgb(16, 48, 67)
    } else {
        Color32::from_rgb(216, 239, 245)
    };
    style.visuals.extreme_bg_color = if dark {
        Color32::from_rgb(10, 35, 53)
    } else {
        Color32::from_rgb(233, 247, 249)
    };
    style.visuals.faint_bg_color = if dark {
        Color32::from_white_alpha(8)
    } else {
        Color32::from_rgb(202, 229, 236)
    };
    style.visuals.selection.bg_fill = Color32::from_rgb(22, 149, 194);
    style.visuals.selection.stroke = Stroke::new(1.0_f32, Color32::WHITE);
    for (state, alpha) in [
        (&mut style.visuals.widgets.inactive, 120),
        (&mut style.visuals.widgets.hovered, 200),
        (&mut style.visuals.widgets.active, 230),
    ] {
        state.bg_fill = if dark {
            Color32::from_rgba_unmultiplied(32, 107, 137, alpha)
        } else {
            Color32::from_rgba_unmultiplied(169, 221, 236, alpha)
        };
        state.weak_bg_fill = state.bg_fill;
        state.bg_stroke = Stroke::new(
            1.0_f32,
            if dark {
                Color32::from_white_alpha(85)
            } else {
                Color32::from_white_alpha(200)
            },
        );
        state.fg_stroke = Stroke::new(1.4_f32, text(dark));
        state.corner_radius = 8.into();
    }
    style.spacing.item_spacing = Vec2::new(10.0, 10.0);
    style.spacing.button_padding = Vec2::new(14.0, 8.0);
    style.spacing.interact_size.y = if cfg!(target_os = "android") {
        44.0
    } else {
        32.0
    };
    style.spacing.slider_width = 180.0;
    style
        .text_styles
        .insert(egui::TextStyle::Body, FontId::proportional(16.0));
    style
        .text_styles
        .insert(egui::TextStyle::Button, FontId::proportional(16.0));
    style
        .text_styles
        .insert(egui::TextStyle::Small, FontId::proportional(13.0));
    style
        .text_styles
        .insert(egui::TextStyle::Heading, FontId::proportional(25.0));
    ctx.set_global_style(style);
}
fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let a = a.to_array();
    let b = b.to_array();
    Color32::from_rgba_premultiplied(
        (a[0] as f32 + (b[0] as f32 - a[0] as f32) * t) as u8,
        (a[1] as f32 + (b[1] as f32 - a[1] as f32) * t) as u8,
        (a[2] as f32 + (b[2] as f32 - a[2] as f32) * t) as u8,
        (a[3] as f32 + (b[3] as f32 - a[3] as f32) * t) as u8,
    )
}
/// A rounded triangle fan keeps wallpaper UVs aligned under each independent glass pane.
fn rounded(
    p: &egui::Painter,
    rect: Rect,
    radius: f32,
    top: Color32,
    bottom: Color32,
    texture: Option<(egui::TextureId, Rect)>,
) {
    let mut mesh = Mesh::default();
    if let Some((id, _)) = texture {
        mesh.texture_id = id;
    }
    let r = radius.min(rect.width() / 2.0).min(rect.height() / 2.0);
    let vertex = |pos: Pos2| {
        let t = ((pos.y - rect.top()) / rect.height()).clamp(0.0, 1.0);
        let uv = texture.map_or(egui::epaint::WHITE_UV, |(_, uv)| {
            Pos2::new(
                uv.left() + ((pos.x - rect.left()) / rect.width()) * uv.width(),
                uv.top() + t * uv.height(),
            )
        });
        Vertex {
            pos,
            uv,
            color: mix(top, bottom, t),
        }
    };
    mesh.vertices.push(vertex(rect.center()));
    for (corner, start) in [
        (Pos2::new(rect.right() - r, rect.top() + r), -90.0_f32),
        (Pos2::new(rect.right() - r, rect.bottom() - r), 0.0),
        (Pos2::new(rect.left() + r, rect.bottom() - r), 90.0),
        (Pos2::new(rect.left() + r, rect.top() + r), 180.0),
    ] {
        for i in 0..=10 {
            let angle = (start + i as f32 * 9.0).to_radians();
            mesh.vertices
                .push(vertex(corner + Vec2::new(angle.cos() * r, angle.sin() * r)));
        }
    }
    let count = mesh.vertices.len() as u32 - 1;
    for i in 1..=count {
        mesh.indices
            .extend_from_slice(&[0, i, if i == count { 1 } else { i + 1 }]);
    }
    p.add(egui::Shape::mesh(mesh));
}
impl Aero {
    pub fn new(ctx: &egui::Context) -> Result<Self> {
        let image =
            image::load_from_memory(include_bytes!("../assets/summer-lake.png"))?.into_rgba8();
        let small =
            image::imageops::resize(&image, 640, 400, image::imageops::FilterType::Triangle);
        let blur = image::imageops::blur(&small, 7.0);
        let load = |name: &str, img: &image::RgbaImage| {
            ctx.load_texture(
                name,
                egui::ColorImage::from_rgba_unmultiplied(
                    [img.width() as usize, img.height() as usize],
                    img.as_raw(),
                ),
                egui::TextureOptions::LINEAR,
            )
        };
        Ok(Self {
            plain: false,
            background: load("summer-lake", &image),
            blurred: load("summer-lake-glass", &blur),
            screen: Rect::NOTHING,
            uv: Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
        })
    }
    pub fn background(&mut self, ui: &Ui, dark: bool) {
        self.screen = ui.max_rect();
        if self.plain {
            ui.painter()
                .rect_filled(self.screen, 0, Color32::from_rgb(235, 238, 242));
            return;
        }
        let ratio = self.screen.aspect_ratio();
        let source = self.background.size_vec2().x / self.background.size_vec2().y;
        let (w, h) = if ratio > source {
            (1.0, source / ratio)
        } else {
            (ratio / source, 1.0)
        };
        self.uv = Rect::from_center_size(Pos2::new(0.5, 0.5), Vec2::new(w, h));
        ui.painter()
            .image(self.background.id(), self.screen, self.uv, Color32::WHITE);
        ui.painter().rect_filled(
            self.screen,
            0,
            if dark {
                Color32::from_rgba_unmultiplied(4, 19, 38, 175)
            } else {
                Color32::from_rgba_unmultiplied(8, 49, 66, 40)
            },
        );
    }
    pub fn glass(&self, p: &egui::Painter, rect: Rect, dark: bool, hover: bool) {
        if self.plain {
            p.rect_filled(
                rect,
                8,
                if hover {
                    Color32::from_rgb(225, 236, 248)
                } else {
                    Color32::WHITE
                },
            );
            p.rect_stroke(
                rect,
                8,
                Stroke::new(1.0_f32, Color32::from_rgb(195, 203, 214)),
                StrokeKind::Inside,
            );
            return;
        }
        for (offset, alpha) in [(8.0, 10), (5.0, 18), (2.0, 27)] {
            p.rect_filled(
                rect.translate(Vec2::new(0.0, offset)).expand(2.0),
                18,
                Color32::from_black_alpha(alpha),
            );
        }
        let to_uv = |pos: Pos2| {
            Pos2::new(
                self.uv.left()
                    + (pos.x - self.screen.left()) / self.screen.width() * self.uv.width(),
                self.uv.top()
                    + (pos.y - self.screen.top()) / self.screen.height() * self.uv.height(),
            )
        };
        let uv = Rect::from_min_max(to_uv(rect.min), to_uv(rect.max));
        rounded(
            p,
            rect,
            16.0,
            Color32::WHITE,
            Color32::WHITE,
            Some((self.blurred.id(), uv)),
        );
        let (top, bottom) = if dark {
            (
                Color32::from_rgba_unmultiplied(19, 66, 90, 218),
                Color32::from_rgba_unmultiplied(5, 31, 53, 235),
            )
        } else {
            (
                Color32::from_rgba_unmultiplied(202, 242, 250, 155),
                Color32::from_rgba_unmultiplied(127, 206, 225, 125),
            )
        };
        rounded(p, rect, 16.0, top, bottom, None);
        if hover {
            rounded(
                p,
                rect,
                16.0,
                Color32::from_white_alpha(60),
                Color32::from_rgba_unmultiplied(52, 222, 255, 65),
                None,
            );
        }
        // Delicate upper reflection and double rim; interior remains readable.
        let shine = Rect::from_min_max(
            rect.min + Vec2::splat(2.0),
            Pos2::new(rect.right() - 2.0, rect.top() + rect.height() * 0.32),
        );
        rounded(
            p,
            shine,
            14.0,
            Color32::from_white_alpha(if hover { 65 } else { 32 }),
            Color32::TRANSPARENT,
            None,
        );
        p.rect_stroke(
            rect,
            16,
            Stroke::new(
                if hover { 1.8_f32 } else { 1.0_f32 },
                Color32::from_white_alpha(if hover { 240 } else { 175 }),
            ),
            StrokeKind::Inside,
        );
        p.rect_stroke(
            rect.shrink(2.0),
            14,
            Stroke::new(0.5_f32, Color32::from_rgba_unmultiplied(78, 168, 191, 100)),
            StrokeKind::Inside,
        );
    }
    pub fn tile(
        &self,
        ui: &mut Ui,
        size: Vec2,
        icon: Icon,
        title: &str,
        subtitle: &str,
        dark: bool,
    ) -> Response {
        let (rect, base) = ui.allocate_exact_size(size, Sense::hover());
        let response = ui
            .interact(rect, ui.id().with(title), Sense::click())
            .union(base);
        self.glass(
            ui.painter(),
            rect,
            dark,
            response.hovered() || response.has_focus(),
        );
        let icon_size = (size.y * 0.44).min(80.0);
        if self.plain {
            ui.painter().circle_filled(
                Pos2::new(rect.center().x, rect.top() + size.y * 0.37),
                icon_size * 0.58,
                Color32::from_rgb(42, 93, 137),
            );
        }
        paint_icon(
            ui.painter(),
            Rect::from_center_size(
                Pos2::new(rect.center().x, rect.top() + size.y * 0.37),
                Vec2::splat(icon_size),
            ),
            icon,
            true,
        );
        ui.painter().text(
            Pos2::new(
                rect.center().x,
                rect.bottom() - if subtitle.is_empty() { 30.0 } else { 47.0 },
            ),
            Align2::CENTER_CENTER,
            title,
            FontId::proportional(if size.x < 180.0 { 17.0 } else { 20.0 }),
            text(dark),
        );
        if !subtitle.is_empty() {
            ui.painter().text(
                Pos2::new(rect.center().x, rect.bottom() - 22.0),
                Align2::CENTER_CENTER,
                subtitle,
                FontId::proportional(13.0),
                muted(dark),
            );
        }
        response.on_hover_cursor(egui::CursorIcon::PointingHand)
    }
}
fn path(p: &egui::Painter, points: Vec<Pos2>, fill: Color32, stroke: Stroke) {
    p.add(egui::Shape::convex_polygon(points, fill, stroke));
}
pub fn paint_icon(p: &egui::Painter, rect: Rect, icon: Icon, bright: bool) {
    let c = rect.center();
    let u = rect.width() / 100.0;
    let pos = |x: f32, y: f32| Pos2::new(rect.left() + x * u, rect.top() + y * u);
    let white = if bright {
        Color32::from_rgb(240, 254, 255)
    } else {
        Color32::from_rgb(172, 197, 207)
    };
    let cyan = if bright {
        Color32::from_rgb(67, 219, 246)
    } else {
        Color32::from_rgb(108, 158, 180)
    };
    let blue = Color32::from_rgb(16, 123, 178);
    let gold = if bright {
        Color32::from_rgb(255, 210, 91)
    } else {
        Color32::from_rgb(133, 164, 179)
    };
    let stroke = Stroke::new(2.0 * u, white);
    match icon {
        Icon::Piano => {
            rounded(
                p,
                rect,
                12.0 * u,
                Color32::from_rgb(139, 231, 248),
                blue,
                None,
            );
            for i in 0..5 {
                let key = Rect::from_min_max(
                    pos(13.0 + i as f32 * 15.0, 13.0),
                    pos(26.0 + i as f32 * 15.0, 87.0),
                );
                p.rect_filled(key, 2, white);
            }
            for i in [0, 1, 3] {
                p.rect_filled(
                    Rect::from_min_max(
                        pos(22.0 + i as f32 * 15.0, 13.0),
                        pos(31.0 + i as f32 * 15.0, 58.0),
                    ),
                    2,
                    Color32::from_rgb(10, 44, 65),
                );
            }
            p.rect_stroke(rect, 12, stroke, StrokeKind::Inside);
        }
        Icon::Library => {
            for side in [0.0, 1.0] {
                let x = 10.0 + 40.0 * side;
                path(
                    p,
                    vec![
                        pos(x, 20.0 + side * 5.0),
                        pos(x + 40.0, 25.0 - side * 5.0),
                        pos(x + 40.0, 87.0),
                        pos(x, 82.0),
                    ],
                    blue,
                    stroke,
                );
                for line in 0..5 {
                    p.line_segment(
                        [
                            pos(x + 8.0, 35.0 + line as f32 * 8.0),
                            pos(x + 32.0, 38.0 + line as f32 * 8.0),
                        ],
                        Stroke::new(1.2 * u, cyan),
                    );
                }
            }
            p.line_segment([pos(50.0, 27.0), pos(50.0, 89.0)], stroke);
        }
        Icon::Course => {
            path(
                p,
                vec![
                    pos(23.0, 44.0),
                    pos(77.0, 44.0),
                    pos(77.0, 72.0),
                    pos(50.0, 85.0),
                    pos(23.0, 72.0),
                ],
                blue,
                stroke,
            );
            path(
                p,
                vec![
                    pos(3.0, 33.0),
                    pos(50.0, 13.0),
                    pos(97.0, 33.0),
                    pos(50.0, 56.0),
                ],
                Color32::from_rgb(65, 169, 211),
                stroke,
            );
            p.line_segment(
                [pos(88.0, 39.0), pos(88.0, 76.0)],
                Stroke::new(3.0 * u, gold),
            );
            p.circle_filled(pos(88.0, 78.0), 4.0 * u, gold);
        }
        Icon::Note => {
            p.line_segment(
                [pos(64.0, 15.0), pos(64.0, 70.0)],
                Stroke::new(7.0 * u, white),
            );
            path(
                p,
                vec![
                    pos(64.0, 15.0),
                    pos(85.0, 29.0),
                    pos(80.0, 44.0),
                    pos(64.0, 32.0),
                ],
                cyan,
                stroke,
            );
            p.add(egui::Shape::ellipse_filled(
                pos(49.0, 75.0),
                Vec2::new(19.0 * u, 13.0 * u),
                white,
            ));
        }
        Icon::Trophy => {
            for x in [10.0, 72.0] {
                p.circle_stroke(pos(x + 9.0, 39.0), 17.0 * u, Stroke::new(5.0 * u, gold));
            }
            path(
                p,
                vec![
                    pos(24.0, 18.0),
                    pos(76.0, 18.0),
                    pos(70.0, 53.0),
                    pos(57.0, 65.0),
                    pos(43.0, 65.0),
                    pos(30.0, 53.0),
                ],
                gold,
                Stroke::new(2.0 * u, white),
            );
            p.line_segment(
                [pos(50.0, 63.0), pos(50.0, 82.0)],
                Stroke::new(8.0 * u, gold),
            );
            p.rect_filled(
                Rect::from_min_max(pos(28.0, 82.0), pos(72.0, 89.0)),
                3,
                gold,
            );
            p.line_segment(
                [pos(33.0, 25.0), pos(37.0, 48.0)],
                Stroke::new(3.0 * u, Color32::from_white_alpha(170)),
            );
        }
        Icon::Chart => {
            for (x, y) in [(15.0, 58.0), (40.0, 36.0), (65.0, 14.0)] {
                rounded(
                    p,
                    Rect::from_min_max(pos(x, y), pos(x + 18.0, 87.0)),
                    3.0 * u,
                    white,
                    cyan,
                    None,
                );
            }
            p.line_segment([pos(10.0, 90.0), pos(90.0, 90.0)], stroke);
        }
        Icon::Settings => {
            let mut pts = vec![];
            for i in 0..40 {
                let a = i as f32 * std::f32::consts::TAU / 40.0;
                let r = if i % 5 < 3 { 39.0 } else { 30.0 };
                pts.push(c + Vec2::angled(a) * r * u);
            }
            p.add(egui::Shape::Path(egui::epaint::PathShape {
                points: pts,
                closed: true,
                fill: Color32::TRANSPARENT,
                stroke: egui::epaint::PathStroke::new(9.0 * u, white),
            }));
            p.circle_filled(c, 25.0 * u, blue);
            p.circle_stroke(c, 14.0 * u, Stroke::new(6.0 * u, cyan));
        }
        Icon::Folder => {
            path(
                p,
                vec![
                    pos(8.0, 23.0),
                    pos(39.0, 23.0),
                    pos(48.0, 34.0),
                    pos(91.0, 34.0),
                    pos(88.0, 85.0),
                    pos(8.0, 85.0),
                ],
                Color32::from_rgb(38, 155, 193),
                stroke,
            );
            path(
                p,
                vec![
                    pos(4.0, 43.0),
                    pos(95.0, 43.0),
                    pos(86.0, 89.0),
                    pos(12.0, 89.0),
                ],
                Color32::from_rgba_unmultiplied(81, 215, 234, 210),
                stroke,
            );
            p.line_segment(
                [pos(16.0, 51.0), pos(82.0, 51.0)],
                Stroke::new(3.0 * u, Color32::from_white_alpha(150)),
            );
        }
        Icon::Star => {
            let pts = (0..10)
                .map(|i| {
                    let a = i as f32 * std::f32::consts::TAU / 10.0 - std::f32::consts::FRAC_PI_2;
                    c + Vec2::angled(a) * if i % 2 == 0 { 42.0 * u } else { 19.0 * u }
                })
                .collect();
            p.add(egui::Shape::Path(egui::epaint::PathShape {
                points: pts,
                closed: true,
                fill: gold,
                stroke: egui::epaint::PathStroke::new(2.0 * u, white),
            }));
        }
    }
}
