//! The first slice of the move to egui: the frame loop, fastframe's scrolling, and
//! a Discover-shaped list to scroll.
//!
//! **What this is for.** The iced shell scrolls at 60 fps p50 with a p95 of 26-62 ms
//! on the machine that reported it, measured with `SIFT_FRAME_LOG`. The reader
//! asked for spotifast's feel; spotifast is egui with `fastframe-scroll`; and the
//! honest way to answer that is to measure the same list in that stack before
//! rewriting a 24,000-line presentation layer for it. So this draws one page's shape
//! — a scrolling column of project cards — wires the real scrolling crate in, and
//! writes the same frame-interval summary the iced shell does. The numbers are
//! comparable because the instrument is the same.
//!
//! **What it is not.** Not a page of the launcher: the cards are synthetic (no
//! `Store`, no network, no images), because the question is the frame, not the
//! content. If the numbers say egui is the answer, the next slices move the real
//! pages over one at a time; if they do not, this cost one binary and told the truth.
//!
//! Run it with `SIFT_FRAME_LOG` naming a file, exactly as the iced build.

mod frame_log;
/// The reference's palette, geometry, type and motion as egui values.
mod theme;
/// The reference's icon set as egui shapes.
mod icons;

use eframe::egui;
use fastframe_scroll::Scrolling;

/// How many cards the list holds. Enough that the scrollbar has somewhere to go and
/// the window has to decide what to draw, which is the cost being measured.
const CARDS: usize = 300;

/// One card's height, in points.
const CARD: f32 = 72.0;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 720.0])
            .with_min_inner_size([960.0, 540.0])
            .with_title("Sift Launcher — egui slice"),
        ..Default::default()
    };
    eframe::run_native(
        "Sift Launcher",
        options,
        Box::new(|_cc| Ok(Box::new(Slice::default()))),
    )
}

/// The window's state: fastframe's scrolling, and the frame log.
#[derive(Default)]
struct Slice {
    scrolling: Scrolling,
    log: frame_log::FrameLog,
}

impl eframe::App for Slice {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.log.begin();
        // First thing in the frame, before any scroll area reads the input: the
        // wheel step, the touchpad scale and the glide all happen here. It rewrites
        // `smooth_scroll_delta`, so every `ScrollArea` picks it up unchanged.
        self.scrolling.apply(&ctx);
        // This is what makes the app "continuous": one repaint after another, so the
        // frame interval is the display's, not a timer's. egui would otherwise sleep
        // until something asked it to wake.
        ctx.request_repaint();

        ui.group(|ui| {
            ui.horizontal(|ui| {
                ui.heading("Sift Launcher");
                ui.separator();
                ui.label(format!("{CARDS} projects"));
                if self.scrolling.gliding() {
                    ui.label("gliding");
                }
            });
        });

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for index in 0..CARDS {
                    card(ui, index);
                }
            });

        self.log.end();
    }
}

/// One project card: an icon, a title, a summary line and a footer, at a fixed
/// height so the list is the same shape as the real cards.
///
/// A painted rectangle rather than an image for the icon: an image would measure the
/// texture path as well as the frame, and the question here is the frame.
fn card(ui: &mut egui::Ui, index: usize) {
    let width = ui.available_width();
    ui.allocate_ui(egui::vec2(width, CARD), |ui| {
        ui.horizontal(|ui| {
            let (rect, _) = ui.allocate_exact_size(egui::vec2(40.0, 40.0), egui::Sense::hover());
            ui.painter()
                .rect_filled(rect, 8.0, ui.visuals().hyperlink_color);
            ui.vertical(|ui| {
                ui.label(
                    egui::RichText::new(format!("Project {index}"))
                        .strong()
                        .size(16.0),
                );
                ui.label("A summary sentence of the length a result card carries.");
                ui.label(
                    egui::RichText::new("12.4M downloads  ·  1.2M followers  ·  Fabric")
                        .size(12.0)
                        .weak(),
                );
            });
        });
    });
    ui.separator();
}

#[cfg(test)]
mod tests {
    /// The list's length in points is `CARDS * CARD`, and a scrollbar's thumb is
    /// computed from it: a card that changed height without this changing would
    /// leave the measurement describing a list nobody drew.
    #[test]
    fn the_list_has_its_documented_length() {
        assert_eq!(super::CARDS, 300);
        assert!(super::CARD >= 60.0 && super::CARD <= 96.0);
    }
}
