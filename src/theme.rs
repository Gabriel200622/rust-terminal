use crate::config::Theme;
use eframe::egui::{self, Color32, FontFamily, FontId, Stroke};

pub fn color(rgb: u32) -> Color32 {
    Color32::from_rgb((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8)
}

#[derive(Clone, Copy)]
pub struct Palette {
    pub bg: Color32,
    pub sidebar: Color32,
    pub raised: Color32,
    pub hover: Color32,
    pub border: Color32,
    pub fg: Color32,
    pub secondary: Color32,
    pub muted: Color32,
    pub accent: Color32,
    pub green: Color32,
    pub selection: Color32,
    pub ansi: [Color32; 16],
}

impl Palette {
    pub fn new(theme: Theme) -> Self {
        let mut p = Self {
            bg: color(0x0f0f11),
            sidebar: color(0x171719),
            raised: color(0x1c1c1f),
            hover: color(0x252529),
            border: color(0x2a2a2f),
            fg: color(0xe4e4eb),
            secondary: color(0xa1a1aa),
            muted: color(0x73737e),
            accent: color(0xb9acf2),
            green: color(0x9bc7a4),
            selection: color(0x2c293b),
            ansi: [
                0x292930, 0xe89292, 0xa3cfa3, 0xe5c890, 0x93b4dd, 0xc6a4db, 0x92c7d0, 0xd9d9e2,
                0x73737e, 0xf5a0a0, 0xb4ddb0, 0xf0d6a3, 0xa8c7ef, 0xd5b6ee, 0xa7dde4, 0xf5f5f7,
            ]
            .map(color),
        };
        match theme {
            Theme::Dusk => {
                p.bg = color(0x16151c);
                p.sidebar = color(0x1d1b25);
                p.raised = color(0x252231);
                p.border = color(0x34303f);
                p.hover = color(0x2f2a3c);
            }
            Theme::Light => {
                p.bg = color(0xfafafa);
                p.sidebar = color(0xf0f0f3);
                p.raised = color(0xffffff);
                p.border = color(0xdededf);
                p.hover = color(0xe6e6ec);
                p.fg = color(0x24242b);
                p.secondary = color(0x5c5c67);
                p.muted = color(0x81818b);
                p.accent = color(0x7160b0);
                p.selection = color(0xe2ddef);
                p.green = color(0x42784b);
                p.ansi = [
                    0x2b2b33, 0xb34b4b, 0x4b784b, 0x916c32, 0x436996, 0x8c5ca3, 0x397780, 0x777782,
                    0x81818b, 0xc65151, 0x518e51, 0xa17731, 0x477eb7, 0x975fb4, 0x318891, 0x24242b,
                ]
                .map(color);
            }
            Theme::Graphite => {}
        }
        p
    }
}

pub fn apply(ctx: &egui::Context, theme: Theme) {
    let p = Palette::new(theme);
    let mut style = egui::Style {
        visuals: if theme == Theme::Light {
            egui::Visuals::light()
        } else {
            egui::Visuals::dark()
        },
        ..Default::default()
    };
    style.visuals.panel_fill = p.bg;
    style.visuals.window_fill = p.sidebar;
    style.visuals.extreme_bg_color = p.bg;
    style.visuals.override_text_color = Some(p.fg);
    style.visuals.selection.bg_fill = p.selection;
    style.visuals.selection.stroke = Stroke::new(1.0, p.accent);
    style.visuals.window_stroke = Stroke::new(1.0, p.border);
    style.visuals.window_corner_radius = egui::CornerRadius::same(12);
    style.visuals.widgets.noninteractive.bg_fill = p.sidebar;
    style.visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, p.border);
    style.visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, p.secondary);
    for w in [
        &mut style.visuals.widgets.inactive,
        &mut style.visuals.widgets.hovered,
        &mut style.visuals.widgets.active,
    ] {
        w.corner_radius = egui::CornerRadius::same(7);
        w.bg_stroke = Stroke::NONE;
    }
    style.visuals.widgets.inactive.bg_fill = p.raised;
    style.visuals.widgets.inactive.weak_bg_fill = p.raised;
    style.visuals.widgets.hovered.bg_fill = p.hover;
    style.visuals.widgets.hovered.weak_bg_fill = p.hover;
    style.visuals.widgets.active.bg_fill = p.selection;
    style.visuals.widgets.active.weak_bg_fill = p.selection;
    style.visuals.widgets.open.bg_fill = p.sidebar;
    style.visuals.widgets.open.weak_bg_fill = p.sidebar;
    style.visuals.widgets.open.fg_stroke = Stroke::new(1.0, p.fg);
    style.visuals.widgets.active.fg_stroke = Stroke::new(1.0, p.accent);
    style.spacing.item_spacing = egui::vec2(8.0, 8.0);
    style.spacing.button_padding = egui::vec2(12.0, 7.0);
    style.spacing.window_margin = egui::Margin::same(16);
    style.spacing.interact_size.y = 28.0;
    style.text_styles.insert(
        egui::TextStyle::Body,
        FontId::new(13.0, FontFamily::Proportional),
    );
    style.text_styles.insert(
        egui::TextStyle::Button,
        FontId::new(12.0, FontFamily::Proportional),
    );
    style.text_styles.insert(
        egui::TextStyle::Small,
        FontId::new(11.0, FontFamily::Proportional),
    );
    style.text_styles.insert(
        egui::TextStyle::Heading,
        FontId::new(18.0, FontFamily::Proportional),
    );
    ctx.set_theme(if theme == Theme::Light {
        egui::Theme::Light
    } else {
        egui::Theme::Dark
    });
    ctx.set_global_style(style);
}

pub use crate::platform::fonts::install as fonts;
