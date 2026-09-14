//! Renderer selection: iced's `wgpu` compositor, or its tiny-skia rasteriser.
//!
//! ## Why this is a probe rather than a constant
//!
//! iced's default candidate order tries `wgpu` first, which is right on a
//! machine whose driver offers a real 3D backend. It is not right everywhere, and
//! the exception is invisible from the outside: `wgpu` does not fail when a
//! machine has no accelerated adapter, it accepts a slower path and reports
//! success. Two rounds of this were measured on the machine this shell was
//! reported against (Intel HD Graphics 4400, driver 20.19.15.5171):
//!
//! | what wgpu offered | adapter | type |
//! |---|---|---|
//! | Direct3D 12 | Microsoft Basic Render Driver | **Cpu** (WARP, software) |
//! | OpenGL | Intel(R) HD Graphics 4400 | Integrated |
//!
//! There is no hardware Direct3D 12 adapter on that machine at all, so the
//! "GPU" path iced was taking was the vendor's **OpenGL** driver. On an idle
//! window it settled at 0% CPU either way; under an identical stream of injected
//! mouse moves it cost **213% of one core against tiny-skia's 69%**, and held
//! **121 MB resident / 464 MB of commit against 22 MB / 12 MB**. A module list
//! cannot see any of this — `d3d12.dll` loads because `wgpu` *probes* the DX12
//! backend, which is exactly the check that once certified the slower path as
//! the faster one.
//!
//! ## Why it asks wgpu instead of DXGI
//!
//! Hand-querying DXGI answers a different question than the one wgpu answers, and
//! the two can disagree. Asking wgpu to enumerate its own adapters means the
//! probe sees precisely the set iced will choose from.
//!
//! ## What it does with the answer
//!
//! Only ever one thing: if *nothing* on this machine qualifies for accelerated
//! compositing, it pins iced to tiny-skia. If something does qualify, iced's own
//! order is left untouched. The change is therefore one-directional — it removes
//! a path that is measurable worse, and never disables one that works today. A
//! machine with a real Direct3D 12 or Vulkan adapter keeps the GPU path exactly
//! as it is now.

use std::ffi::OsStr;
use std::sync::OnceLock;

/// iced's own switch. Honoured verbatim when set, so support can override this
/// decision without a rebuild.
pub const BACKEND_ENV: &str = "ICED_BACKEND";

/// iced's software rasteriser, and the answer when nothing else qualifies.
pub const SOFTWARE_BACKEND: &str = "tiny-skia";

/// Backends this shell is willing to call accelerated.
///
/// Direct3D 12 and Vulkan are the drivers' real 3D paths on Windows. `wgpu`'s
/// OpenGL backend is deliberately not on this list: it routes through the
/// vendor's legacy GL driver, and on the machine measured above it was about
/// three times the CPU and thirty-eight times the committed memory of
/// rasterising on the CPU directly.
pub const ACCELERATED_BACKENDS: [&str; 2] = ["dx12", "vulkan"];

/// One adapter wgpu reported.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Adapter {
    /// Driver-reported name, e.g. `Intel(R) HD Graphics 4400`.
    pub name: String,
    /// Backend that exposed it, e.g. `Direct3D 12`.
    pub backend: String,
    /// wgpu's raw backend key (`dx12`, `vulkan`, `gl`, …), which is what the
    /// qualification rule is written against.
    pub backend_key: String,
    /// Whether wgpu considers it real hardware rather than a CPU rasteriser.
    pub hardware: bool,
}

impl Adapter {
    /// Whether this adapter is one the shell will render on.
    pub fn qualifies(&self) -> bool {
        self.hardware && ACCELERATED_BACKENDS.contains(&self.backend_key.as_str())
    }

    /// One line for the About page: `name · backend`, marked when the adapter is
    /// a software one, because that is the fact most likely to explain an
    /// unexpectedly heavy frame.
    pub fn summary(&self) -> String {
        if self.hardware {
            format!("{} · {}", self.name, self.backend)
        } else {
            format!("{} · {} (software)", self.name, self.backend)
        }
    }
}

/// Everything the probe learned, kept whole so the About page can print it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Report {
    /// Every adapter wgpu enumerated, in wgpu's own order.
    pub adapters: Vec<Adapter>,
    /// The first adapter that qualifies for accelerated compositing, if any.
    pub accelerated: Option<Adapter>,
}

impl Report {
    /// Whether GPU compositing is available on this machine.
    pub fn accelerated_available(&self) -> bool {
        self.accelerated.is_some()
    }

    /// The most capable *hardware* adapter found, whether or not it qualifies.
    ///
    /// Used for reporting only. wgpu lists software adapters first on Windows, so
    /// "the first adapter" is the one adapter a user is least likely to care
    /// about.
    pub fn best_hardware(&self) -> Option<&Adapter> {
        self.adapters.iter().find(|adapter| adapter.hardware)
    }

    /// One line for the About page.
    ///
    /// Names the adapter that will be used, or — when none will be — the GPU the
    /// machine actually has, alongside the fact that software rendering was
    /// chosen. Naming the *first* adapter here would be actively misleading: on
    /// the machine measured above, wgpu enumerates WARP before the real GPU, so
    /// the first entry is the software rasteriser.
    pub fn summary(&self) -> String {
        if let Some(adapter) = &self.accelerated {
            return adapter.summary();
        }
        match self.best_hardware() {
            Some(adapter) => format!("{} — software rendering", adapter.summary()),
            None => "software rendering (no graphics adapter detected)".to_string(),
        }
    }
}

/// wgpu's backend name, spelled the way the rest of the shell writes it.
///
/// Built from wgpu's own `to_str` rather than by matching the enum, so a backend
/// added in a future wgpu cannot break this build.
pub fn backend_name(backend: &str) -> String {
    match backend {
        "dx12" => "Direct3D 12".to_string(),
        "vulkan" => "Vulkan".to_string(),
        "metal" => "Metal".to_string(),
        "gl" => "OpenGL".to_string(),
        "webgpu" => "WebGPU".to_string(),
        "empty" => "none".to_string(),
        other => other.to_string(),
    }
}

/// Whether wgpu calls an adapter real hardware.
///
/// `Cpu` is wgpu's own label for a software rasteriser (WARP on Windows) and the
/// only device type that means "no GPU here". `Other` counts as hardware: it
/// means a real adapter wgpu could not classify, not a CPU one.
pub fn is_hardware(device_type: wgpu::DeviceType) -> bool {
    device_type != wgpu::DeviceType::Cpu
}

/// Pick the adapter to report as accelerated, preferring a discrete GPU over an
/// integrated one and keeping wgpu's order within a class.
pub fn best_qualifying(adapters: &[Adapter]) -> Option<Adapter> {
    let mut fallback: Option<&Adapter> = None;
    for adapter in adapters {
        if !adapter.qualifies() {
            continue;
        }
        if adapter.backend_key == "dx12" {
            // Direct3D 12 first: on Windows it is the path iced is built and
            // tested against, and it avoids a translation layer.
            return Some(adapter.clone());
        }
        fallback.get_or_insert(adapter);
    }
    fallback.cloned()
}

/// Enumerate every adapter wgpu can see on this machine.
pub fn enumerate() -> Vec<Adapter> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::all(),
        ..Default::default()
    });

    instance
        .enumerate_adapters(wgpu::Backends::all())
        .into_iter()
        .map(|adapter| {
            let info = adapter.get_info();
            let key = info.backend.to_str().to_string();
            Adapter {
                name: info.name,
                backend: backend_name(&key),
                backend_key: key,
                hardware: is_hardware(info.device_type),
            }
        })
        .collect()
}

/// Probe this machine.
///
/// Never returns an error: a machine with no adapter at all is a normal outcome
/// (a headless build box, or a session that cannot reach the GPU), and it is the
/// same case, for this purpose, as a machine whose only adapter is software.
pub fn probe() -> Report {
    let adapters = enumerate();
    let accelerated = best_qualifying(&adapters);
    Report { adapters, accelerated }
}

/// The `ICED_BACKEND` value to force, or `None` to leave iced's order in place.
///
/// An explicit value always wins — that is the documented iced switch, and it is
/// how a machine opts back in without a rebuild.
pub fn forced_backend(explicit: Option<&OsStr>, report: &Report) -> Option<&'static str> {
    match explicit {
        Some(value) if !value.is_empty() => None,
        _ if report.accelerated_available() => None,
        _ => Some(SOFTWARE_BACKEND),
    }
}

/// What the probe found, once the process has run [`select_renderer`].
///
/// Kept here rather than threaded through the application state because the
/// answer is fixed for the life of the process and only the About page reads it.
static DETECTED: OnceLock<Report> = OnceLock::new();

/// The report produced by [`select_renderer`], if it has run.
pub fn detected() -> Option<&'static Report> {
    DETECTED.get()
}

/// Which renderer this process is running, as far as it can know from here.
///
/// When nothing pins `ICED_BACKEND`, iced tries `wgpu` first and would fall back
/// only if no compositor could be built — so this reports what was *asked for*,
/// and [`detected`] carries the adapter evidence behind the request. It is
/// deliberately not phrased as a certainty about the frame that just got drawn:
/// nothing inside the application can observe which compositor won.
pub fn active_backend() -> &'static str {
    match std::env::var(BACKEND_ENV) {
        Ok(value) if value == SOFTWARE_BACKEND => "tiny-skia (CPU rasteriser)",
        _ => "wgpu (GPU compositor)",
    }
}

/// Probe the machine and pin `ICED_BACKEND` accordingly.
///
/// Must run before iced builds its compositor, which happens inside `App::run`;
/// setting the variable here is early enough and needs no iced API. Returns the
/// report, for tests and for the About page.
pub fn select_renderer() -> Report {
    let report = probe();
    if let Some(backend) = forced_backend(std::env::var_os(BACKEND_ENV).as_deref(), &report) {
        std::env::set_var(BACKEND_ENV, backend);
    }
    DETECTED.get_or_init(|| report).clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn adapter(name: &str, key: &str, hardware: bool) -> Adapter {
        Adapter {
            name: name.to_string(),
            backend: backend_name(key),
            backend_key: key.to_string(),
            hardware,
        }
    }

    fn report(adapters: Vec<Adapter>) -> Report {
        let accelerated = best_qualifying(&adapters);
        Report { adapters, accelerated }
    }

    /// The adapter set measured on the machine this shell was reported against.
    /// Both rounds of this bug were decided by what is *in* this list, so it is
    /// the fixture the rules are tested against.
    fn the_reported_machine() -> Report {
        report(vec![
            adapter("Microsoft Basic Render Driver", "dx12", false),
            adapter("Intel(R) HD Graphics 4400", "gl", true),
        ])
    }

    #[test]
    fn only_a_software_adapter_is_not_hardware() {
        assert!(!is_hardware(wgpu::DeviceType::Cpu), "a CPU rasteriser is not a GPU");
        for real in [
            wgpu::DeviceType::DiscreteGpu,
            wgpu::DeviceType::IntegratedGpu,
            wgpu::DeviceType::VirtualGpu,
            wgpu::DeviceType::Other,
        ] {
            assert!(is_hardware(real), "{real:?} must count as hardware");
        }
    }

    #[test]
    fn hardware_on_a_legacy_backend_does_not_qualify() {
        // The measured case: an integrated GPU reachable only over OpenGL. It is
        // real hardware, and it is still the slower of the two renderers, so
        // "hardware" alone must not be the rule.
        let intel = adapter("Intel(R) HD Graphics 4400", "gl", true);
        assert!(intel.hardware);
        assert!(!intel.qualifies(), "the legacy GL path must not qualify");
    }

    #[test]
    fn software_on_a_modern_backend_does_not_qualify() {
        // WARP exposes Direct3D 12 and is still a CPU rasteriser.
        let warp = adapter("Microsoft Basic Render Driver", "dx12", false);
        assert!(!warp.qualifies());
    }

    #[test]
    fn hardware_on_direct3d_12_or_vulkan_qualifies() {
        assert!(adapter("GeForce RTX 4070", "dx12", true).qualifies());
        assert!(adapter("AMD Radeon", "vulkan", true).qualifies());
        // Metal is a real 3D path, just not one this shell ships on: it is not
        // in the accelerated list because Windows never exposes it.
        assert!(!adapter("Apple M2", "metal", true).qualifies());
    }

    #[test]
    fn the_reported_machine_is_pinned_to_the_rasteriser() {
        let report = the_reported_machine();
        assert!(!report.accelerated_available(), "nothing there qualifies");
        assert!(
            report.summary().contains("software rendering"),
            "the summary must not imply GPU compositing: {}",
            report.summary()
        );
        assert_eq!(forced_backend(None, &report), Some("tiny-skia"));
    }

    #[test]
    fn a_machine_with_a_real_backend_keeps_iceds_own_order() {
        let machine = report(vec![adapter("GeForce RTX 4070", "dx12", true)]);
        assert!(machine.accelerated_available());
        assert_eq!(
            forced_backend(None, &machine),
            None,
            "a working GPU path must be left alone"
        );
    }

    #[test]
    fn a_machine_with_nothing_is_also_pinned() {
        assert_eq!(forced_backend(None, &Report::default()), Some("tiny-skia"));
    }

    #[test]
    fn direct3d_12_is_preferred_when_both_are_available() {
        let machine = report(vec![
            adapter("Radeon Vega", "vulkan", true),
            adapter("GeForce RTX 4070", "dx12", true),
        ]);
        assert_eq!(machine.accelerated.unwrap().backend_key, "dx12");
    }

    #[test]
    fn an_explicit_backend_is_never_overridden() {
        let target = the_reported_machine();
        for explicit in ["wgpu", "tiny-skia", "gpu", "software", "anything-else"] {
            assert_eq!(
                forced_backend(Some(std::ffi::OsStr::new(explicit)), &target),
                None,
                "{explicit} was overridden, so the documented iced switch is broken"
            );
        }
    }

    #[test]
    fn an_empty_backend_variable_still_gets_a_decision() {
        // `ICED_BACKEND=` is a shell accident, not a choice. iced treats the
        // empty string as a backend name matching nothing and quietly falls back
        // to its own order, which is the behaviour being avoided.
        assert_eq!(
            forced_backend(Some(std::ffi::OsStr::new("")), &the_reported_machine()),
            Some("tiny-skia")
        );
    }

    #[test]
    fn backends_are_named_for_people() {
        assert_eq!(backend_name("dx12"), "Direct3D 12");
        assert_eq!(backend_name("gl"), "OpenGL");
        // Anything wgpu grows later passes through instead of failing to build.
        assert_eq!(backend_name("something-new"), "something-new");
    }

    #[test]
    fn the_report_names_the_gpu_rather_than_the_software_adapter() {
        // wgpu enumerates WARP *before* the real GPU, so taking the first entry
        // reported "Microsoft Basic Render Driver" for a machine whose GPU is an
        // Intel HD 4400. The report has to name the hardware the user has.
        let machine = the_reported_machine();
        let summary = machine.summary();
        assert!(
            summary.contains("Intel(R) HD Graphics 4400"),
            "the summary should name the real GPU: {summary}"
        );
        assert!(
            !summary.contains("Basic Render Driver"),
            "and not the software rasteriser it is declining to use: {summary}"
        );
        assert!(summary.contains("software"), "while saying it will not be used: {summary}");
    }

    #[test]
    fn a_machine_with_no_adapter_says_so() {
        assert!(Report::default().summary().contains("no graphics adapter"));
        assert!(Report::default().best_hardware().is_none());
    }

    /// Informational: run with `--nocapture` to see what this machine offers.
    /// Asserts nothing, because a build machine may legitimately have no GPU.
    #[test]
    fn report_what_this_machine_offers() {
        let report = probe();
        println!("--- every adapter wgpu can see ---");
        for adapter in &report.adapters {
            println!(
                "  {} | {} | hardware={} | qualifies={}",
                adapter.name,
                adapter.backend,
                adapter.hardware,
                adapter.qualifies()
            );
        }
        println!("--- decision ---");
        println!(
            "  summary: {}",
            report.summary()
        );
        println!(
            "  accelerated: {} | forced backend: {:?}",
            report.accelerated_available(),
            forced_backend(None, &report)
        );
    }
}
