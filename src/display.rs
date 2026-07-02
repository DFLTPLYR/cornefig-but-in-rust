use core::fmt::Write as _;

use embedded_graphics::image::{Image, ImageRaw};
use embedded_graphics::mono_font::MonoTextStyle;
use embedded_graphics::mono_font::ascii::FONT_5X8;
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::{Circle, Line, PrimitiveStyle, Rectangle};
use embedded_graphics::text::Text;
use rmk::display::{DisplayRenderer, RenderContext};
use rmk::event::BatteryStatusEvent;
use rmk::types::battery::BatteryStatus;

const FONT_STYLE: MonoTextStyle<'_, BinaryColor> = MonoTextStyle::new(&FONT_5X8, BinaryColor::On);
const STROKE: PrimitiveStyle<BinaryColor> = PrimitiveStyle::with_stroke(BinaryColor::On, 1);
const FILL: PrimitiveStyle<BinaryColor> = PrimitiveStyle::with_fill(BinaryColor::On);

const ICON_SZ: i32 = 8;
const MOD_GAP: i32 = 3;
const LOCK_DOT_DIAMETER: u32 = 4;
const LOCK_DOT_SPACING: i32 = LOCK_DOT_DIAMETER as i32 + 2;

#[derive(Clone, Copy)]
enum Orientation {
    Landscape,
    Portrait,
}

#[allow(dead_code)]
struct Layout {
    orientation: Orientation,
    w: i32,
    h: i32,
    zones: [i32; 5],
}

#[allow(dead_code)]
impl Layout {
    fn from_display<D: DrawTarget>(display: &D) -> Self {
        let bbox = display.bounding_box();
        let w = bbox.size.width as i32;
        let h = bbox.size.height as i32;

        let (orientation, zones) = if w >= h {
            let z = h / 3;
            (Orientation::Landscape, [0, z, 2 * z, h, h])
        } else {
            let z = h / 4;
            (Orientation::Portrait, [0, z, 2 * z, 3 * z, h])
        };

        Self {
            orientation,
            w,
            h,
            zones,
        }
    }

    fn zone_center_y(&self, z: usize) -> i32 {
        let top = self.zones[z];
        let bot = self.zones[z + 1];
        top + (bot - top) / 2
    }

    fn zone_top(&self, z: usize) -> i32 {
        self.zones[z]
    }

    fn zone_height(&self, z: usize) -> i32 {
        self.zones[z + 1] - self.zones[z]
    }

    fn center_x(&self, char_count: usize) -> i32 {
        ((self.w - char_count as i32 * 5) / 2).max(0)
    }
}

/// Column widths for the 32px-wide display.
const COL_W0: i32 = 8;
const COL_W1: i32 = 16;
const COL_W2: i32 = 8;

#[allow(dead_code)]
#[derive(Clone, Copy)]
pub enum LayoutDirection {
    LeftToRight,
    RightToLeft,
}

#[derive(Clone, Copy)]
pub struct SideConfig {
    pub direction: LayoutDirection,
    pub show_battery: bool,
    pub show_wpm: bool,
    pub show_layer: bool,
    pub show_mods: bool,
    pub show_ble: bool,
    pub show_locks: bool,
}

#[allow(dead_code)]
impl SideConfig {
    fn col_header_x(&self) -> i32 {
        match self.direction {
            LayoutDirection::LeftToRight => 0,
            LayoutDirection::RightToLeft => COL_W1 + COL_W2,
        }
    }
    fn col_content_x(&self) -> i32 {
        match self.direction {
            LayoutDirection::LeftToRight | LayoutDirection::RightToLeft => COL_W0,
        }
    }
    fn col_footer_x(&self) -> i32 {
        match self.direction {
            LayoutDirection::LeftToRight => COL_W0 + COL_W1,
            LayoutDirection::RightToLeft => 0,
        }
    }
}

pub struct ColumnLayout {
    config: SideConfig,
}

impl Default for ColumnLayout {
    fn default() -> Self {
        Self {
            config: SideConfig {
                direction: LayoutDirection::LeftToRight,
                show_battery: true,
                show_wpm: true,
                show_layer: true,
                show_mods: true,
                show_ble: true,
                show_locks: true,
            },
        }
    }
}

/// Central-side defaults: left-to-right.
#[allow(dead_code)]
pub struct CentralLayout(ColumnLayout);

/// Peripheral-side defaults: right-to-left.
#[allow(dead_code)]
pub struct PeripheralLayout(ColumnLayout);

impl Default for CentralLayout {
    fn default() -> Self {
        Self(ColumnLayout {
            config: SideConfig {
                direction: LayoutDirection::LeftToRight,
                show_battery: false,
                show_wpm: true,
                show_layer: true,
                show_mods: true,
                show_ble: false,
                show_locks: false,
            },
        })
    }
}

impl Default for PeripheralLayout {
    fn default() -> Self {
        Self(ColumnLayout {
            config: SideConfig {
                direction: LayoutDirection::RightToLeft,
                show_battery: false,
                show_wpm: false,
                show_layer: true,
                show_mods: true,
                show_ble: false,
                show_locks: false,
            },
        })
    }
}

impl DisplayRenderer<BinaryColor> for ColumnLayout {
    fn render<D: DrawTarget<Color = BinaryColor>>(&mut self, ctx: &RenderContext, display: &mut D) {
        display.clear(BinaryColor::Off).ok();
        if ctx.sleeping {
            return;
        }
        let layout = Layout::from_display(display);
        draw_header_col(ctx, display, &self.config, &layout);
        draw_content_col(ctx, display, &self.config, &layout);
        draw_footer_col(ctx, display, &self.config, &layout);
    }
}

impl DisplayRenderer<BinaryColor> for CentralLayout {
    fn render<D: DrawTarget<Color = BinaryColor>>(&mut self, ctx: &RenderContext, display: &mut D) {
        self.0.render(ctx, display)
    }
}

impl DisplayRenderer<BinaryColor> for PeripheralLayout {
    fn render<D: DrawTarget<Color = BinaryColor>>(&mut self, ctx: &RenderContext, display: &mut D) {
        self.0.render(ctx, display)
    }
}

// -- Drawing functions --

fn draw_header_col<D: DrawTarget<Color = BinaryColor>>(
    ctx: &RenderContext,
    display: &mut D,
    config: &SideConfig,
    layout: &Layout,
) {
    if config.show_layer {
        let mut buf: heapless::String<4> = heapless::String::new();
        write!(buf, "{}", ctx.layer).ok();
        Text::new(
            &buf,
            Point::new(layout.center_x(1), layout.zone_center_y(0) - 4),
            FONT_STYLE,
        )
        .draw(display)
        .ok();
    }
    if config.show_wpm {
        let mut wpm_buf: heapless::String<4> = heapless::String::new();
        write!(wpm_buf, "{:03}", ctx.wpm).ok();
        Text::new(
            &wpm_buf,
            Point::new(layout.center_x(3), layout.zone_center_y(1) - 4),
            FONT_STYLE,
        )
        .draw(display)
        .ok();
    }
}

fn draw_content_col<D: DrawTarget<Color = BinaryColor>>(
    ctx: &RenderContext,
    display: &mut D,
    config: &SideConfig,
    layout: &Layout,
) {
    if config.show_mods {
        let m = ctx.modifiers;
        let mods: [(&[u8; 8], bool); 4] = [
            (&SHIFT, m.left_shift() || m.right_shift()),
            (&CTRL, m.left_ctrl() || m.right_ctrl()),
            (&ALT, m.left_alt() || m.right_alt()),
            (&GUI, m.left_gui() || m.right_gui()),
        ];

        let total_w = 2 * ICON_SZ + MOD_GAP;
        let start_x = config.col_content_x() + (COL_W1 - total_w) / 2;

        let row_h = ICON_SZ + 4;
        let content_cy = layout.zone_center_y(2);
        let y1 = content_cy - row_h / 2;
        let y2 = y1 + row_h;

        draw_mod_pair(display, &mods[..2], start_x, y1);
        draw_mod_pair(display, &mods[2..], start_x, y2);
    }
}

fn draw_mod_pair<D: DrawTarget<Color = BinaryColor>>(display: &mut D, mods: &[(&[u8; 8], bool)], x: i32, y: i32) {
    for (i, (icon_data, active)) in mods.iter().enumerate() {
        let px = x + i as i32 * (ICON_SZ + MOD_GAP);
        let raw: ImageRaw<BinaryColor> = ImageRaw::new(*icon_data, 8);
        Image::new(&raw, Point::new(px, y)).draw(display).ok();

        if *active {
            let underline_y = y + ICON_SZ + 1;
            Line::new(Point::new(px, underline_y), Point::new(px + ICON_SZ - 1, underline_y))
                .into_styled(STROKE)
                .draw(display)
                .ok();
        }
    }
}

fn draw_footer_col<D: DrawTarget<Color = BinaryColor>>(
    ctx: &RenderContext,
    display: &mut D,
    config: &SideConfig,
    layout: &Layout,
) {
    let cx = config.col_footer_x() + COL_W2 / 2;

    if config.show_ble {
        draw_ble_indicator(ctx, display, cx, layout);
    }
    if config.show_locks {
        let lock_x = cx - LOCK_DOT_DIAMETER as i32 / 2;
        draw_lock_dots(ctx, display, lock_x, layout.zone_center_y(3) - 4);
    }
    if config.show_battery {
        draw_battery_icon(ctx.battery, display, config.col_footer_x(), layout);
    }
}

fn draw_ble_indicator<D: DrawTarget<Color = BinaryColor>>(
    ctx: &RenderContext,
    display: &mut D,
    cx: i32,
    layout: &Layout,
) {
    let connected = is_connected(ctx);
    let y = layout.zone_center_y(3) - 4;
    let check_x = cx - 2;
    if connected {
        Line::new(Point::new(check_x, y + 2), Point::new(check_x + 2, y + 4))
            .into_styled(STROKE)
            .draw(display)
            .ok();
        Line::new(Point::new(check_x + 2, y + 4), Point::new(check_x + 4, y))
            .into_styled(STROKE)
            .draw(display)
            .ok();
    } else {
        Line::new(Point::new(check_x, y), Point::new(check_x + 4, y + 4))
            .into_styled(STROKE)
            .draw(display)
            .ok();
        Line::new(Point::new(check_x + 4, y), Point::new(check_x, y + 4))
            .into_styled(STROKE)
            .draw(display)
            .ok();
    }
}

fn draw_lock_dots<D: DrawTarget<Color = BinaryColor>>(ctx: &RenderContext, display: &mut D, x: i32, y: i32) {
    if ctx.caps_lock {
        Circle::new(Point::new(x, y), LOCK_DOT_DIAMETER)
            .into_styled(FILL)
            .draw(display)
            .ok();
    }
    if ctx.num_lock {
        Circle::new(Point::new(x, y + LOCK_DOT_SPACING), LOCK_DOT_DIAMETER)
            .into_styled(FILL)
            .draw(display)
            .ok();
    }
}

fn draw_battery_icon<D: DrawTarget<Color = BinaryColor>>(
    battery: BatteryStatusEvent,
    display: &mut D,
    col_x: i32,
    layout: &Layout,
) {
    const NUM_BARS: i32 = 6;
    const BODY_W: i32 = 5;
    const BODY_H: i32 = NUM_BARS + 2;
    const NUB_W: i32 = 3;
    const NUB_H: i32 = 1;
    const BAR_H: i32 = 1;

    let body_x = col_x + (COL_W2 - BODY_W) / 2;
    let nub_x = body_x + (BODY_W - NUB_W) / 2;
    let top_y = layout.zone_top(4) - BODY_H - 2;
    let body_y = top_y + NUB_H;

    Rectangle::new(Point::new(nub_x, top_y), Size::new(NUB_W as u32, NUB_H as u32))
        .into_styled(STROKE)
        .draw(display)
        .ok();
    Rectangle::new(Point::new(body_x, body_y), Size::new(BODY_W as u32, BODY_H as u32))
        .into_styled(STROKE)
        .draw(display)
        .ok();

    let bars: i32 = match *battery {
        BatteryStatus::Available { level: Some(pct), .. } => ((pct as i32 * NUM_BARS) + 99) / 100,
        BatteryStatus::Available { level: None, .. } => NUM_BARS,
        BatteryStatus::Unavailable => 0,
    };

    for i in 0..bars {
        let bar_y = body_y + BODY_H - 1 - (i + 1) * BAR_H;
        Rectangle::new(
            Point::new(body_x + 1, bar_y),
            Size::new((BODY_W - 2) as u32, BAR_H as u32),
        )
        .into_styled(FILL)
        .draw(display)
        .ok();
    }
}

fn is_connected(ctx: &RenderContext) -> bool {
    ctx.central_connected || ctx.ble_status.state == rmk::types::ble::BleState::Connected
}

// --- Modifier icons (8x8 pixel bitmaps) ---

const SHIFT: [u8; 8] = [0x18, 0x3C, 0x7E, 0x18, 0x18, 0x18, 0x18, 0x00];

const CTRL: [u8; 8] = [0x00, 0x18, 0x3C, 0x66, 0xC3, 0x00, 0x00, 0x00];

const ALT: [u8; 8] = [0x18, 0x24, 0x42, 0x7E, 0x42, 0x42, 0x00, 0x00];

const GUI: [u8; 8] = [0x00, 0x6C, 0x6C, 0x00, 0x6C, 0x6C, 0x00, 0x00];
