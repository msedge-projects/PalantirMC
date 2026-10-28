//! Sift Launcher's interface, in egui.
//!
//! **Why egui.** The frame log measured iced at a 26-62 ms p95 against egui's 17.6-18.6 ms
//! on the same machine, same window, same list -- egui re-emits everything every frame and
//! still holds a tighter tail, because the cost is what a toolkit does between the widget
//! and the present. And the interface this launcher wants next -- dynamic elements that
//! morph in place -- is a function of state in immediate mode and a subtree diff in a
//! retained tree.
//!
//! **Why the numbers in `geometry` are measured.** Every element is placed with a
//! rectangle read off a capture of the Modrinth app with `tools/parity/edges.py`; the
//! parity requirement is +/-2 px, and a layout engine's own idea of spacing is exactly the
//! thing that would miss it.
//!
//! **Why it draws nothing when idle.** The window asks for a frame while a hover, a glide
//! or a transition is in flight and stops when it lands. The slice this grew from called
//! `ctx.request_repaint()` every frame, which is the behaviour `docs/LIGHTNESS.md` names as
//! the idle-CPU budget's failure mode.

/// The product's mark, embedded and uploaded once: the exe is the product, so its logo
/// travels inside it rather than beside it.
mod brand;
mod data;
mod fonts;
mod frame_log;
mod game;
mod geometry;
mod icons;
mod install;
mod pages;
mod panel;
mod images;
mod nbt;
mod search;
mod shell;
/// The reference's palette, geometry, type and motion as egui values.
mod theme;
mod widgets;

use eframe::egui;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([geometry::WINDOW.0, geometry::WINDOW.1])
            .with_min_inner_size([960.0, 540.0])
            .with_title("Sift Launcher"),
        ..Default::default()
    };
    eframe::run_native(
        "Sift Launcher",
        options,
        Box::new(|cc| {
            // The generated tokens, not egui's defaults: the palette is the reference's.
            let shell = shell::Shell::default();
            // Inter at the reference's weights, then the reference's palette: a theme
            // without its type is half a theme, and the first capture showed the gap.
            fonts::install(&cc.egui_ctx);
            // The mark's texture, uploaded before the first frame, so the title bar's first
            // paint is the product's logo rather than the plate that stands in for it. Both
            // calls are process-wide setup and neither depends on the other.
            brand::install(&cc.egui_ctx);
            cc.egui_ctx.set_visuals(shell.theme.colors());
            Ok(Box::new(App {
                shell,
                log: frame_log::FrameLog::default(),
            }))
        }),
    )
}

/// The application: one shell, and the frame log when somebody asked for it.
struct App {
    shell: shell::Shell,
    log: frame_log::FrameLog,
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.log.begin();
        let animating = self.shell.ui(ui);
        self.log.end();
        // Only while something is in flight. An idle window draws nothing at all, which is
        // both the budget in docs/LIGHTNESS.md and the reason a fan does not spin.
        if animating {
            ctx.request_repaint();
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::shell;

    /// The shell opens on Home, like the reference: a launcher that opened on a catalogue
    /// would look like a store. This test was written when Discover was the default, and it
    /// caught the change rather than the change quietly passing.
    #[test]
    fn a_fresh_shell_opens_on_home() {
        let shell = shell::Shell::default();
        assert_eq!(shell.rail, shell::Rail::Home);
    }
}
