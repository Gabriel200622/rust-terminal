#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
use neptune_terminal::{Launch, app, config, persistence::window_state};

fn main() -> anyhow::Result<()> {
    let mut launch = Launch::default();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--cwd" => {
                launch.cwd = Some(
                    args.next()
                        .ok_or_else(|| anyhow::anyhow!("--cwd needs a path"))?
                        .into(),
                )
            }
            "--ssh" => {
                let destination = args.next().ok_or_else(|| {
                    anyhow::anyhow!("--ssh needs a destination such as user@host")
                })?;
                neptune_model::Remote::parse(&destination)
                    .map_err(|error| anyhow::anyhow!("--ssh {destination:?}: {error}"))?;
                launch.ssh = Some(destination);
            }
            "--config" => {
                launch.config = Some(
                    args.next()
                        .ok_or_else(|| anyhow::anyhow!("--config needs a path"))?
                        .into(),
                )
            }
            "--data-root" => {
                launch.data_root = Some(
                    args.next()
                        .ok_or_else(|| anyhow::anyhow!("--data-root needs a path"))?
                        .into(),
                );
            }
            "--command" => {
                launch.command = Some(
                    args.next()
                        .ok_or_else(|| anyhow::anyhow!("--command needs a shell command"))?,
                )
            }
            "--screenshot" => {
                launch.screenshot = Some(
                    args.next()
                        .ok_or_else(|| anyhow::anyhow!("--screenshot needs a path"))?
                        .into(),
                )
            }
            "--size" => {
                let size = args
                    .next()
                    .ok_or_else(|| anyhow::anyhow!("--size needs WIDTHxHEIGHT"))?;
                let (w, h) = size
                    .split_once('x')
                    .ok_or_else(|| anyhow::anyhow!("--size needs WIDTHxHEIGHT"))?;
                let w: f32 = w.parse()?;
                let h: f32 = h.parse()?;
                anyhow::ensure!(
                    window_state::WindowState::valid_size([w, h]),
                    "size must be 640x400 to 8192x8192"
                );
                launch.size = Some([w, h]);
            }
            "--no-restore" => launch.no_restore = true,
            "--diagnostics" => launch.diagnostics = true,
            "--version" | "-V" => {
                println!("Neptune {}", env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            "--help" | "-h" => {
                println!(
                    "Neptune — a native GPU terminal\n\nUsage: neptune [OPTIONS]\n  --cwd PATH         Open a workspace at PATH\n  --ssh DESTINATION  Open a workspace whose terminals run on an SSH host\n  --config PATH      Use a TOML configuration\n  --data-root PATH   Isolate settings and saved workspace/window state\n  --command COMMAND  Run a command in the first terminal\n  --no-restore       Start without saved workspaces\n  --size WIDTHxHEIGHT Override saved window size and maximized state\n  --screenshot PATH  Capture the native window after 3 seconds and exit\n  --diagnostics      Print renderer and display details\n  --version\n  --help"
                );
                return Ok(());
            }
            _ => anyhow::bail!("Unknown option {arg}. Try --help"),
        }
    }
    if let Some(cwd) = &launch.cwd {
        anyhow::ensure!(cwd.is_dir(), "Directory does not exist: {}", cwd.display());
    }
    // A command is typed into the first terminal shortly after it starts. Over
    // SSH that could be a password or host-key prompt rather than a shell.
    anyhow::ensure!(
        launch.ssh.is_none() || launch.command.is_none(),
        "--command cannot be combined with --ssh"
    );
    let data_root = launch.data_root.clone();
    let ephemeral = launch.screenshot.is_some();
    // Resolve/migrate storage and read geometry before creating the window.
    // Failure must preserve old work rather than start saving into a new root.
    let (data, mut window) = std::thread::Builder::new()
        .name("neptune-window-restore".into())
        .spawn(move || -> anyhow::Result<_> {
            let data = match data_root {
                Some(data) => data,
                None if ephemeral => config::data_dir(),
                None => config::prepare_data_dir()?,
            };
            let window = if ephemeral {
                window_state::LoadReport::default()
            } else {
                window_state::load(&data.join("window.json"))
            };
            Ok((data, window))
        })?
        .join()
        .map_err(|_| {
            anyhow::anyhow!("Storage restoration worker stopped; saved data is preserved")
        })??;
    launch.data_root = Some(data);
    if let Some(size) = launch.size {
        window.state.inner_size = size;
        window.state.maximized = false;
    }
    eframe::run_native(
        "Neptune",
        native_options(&window.state),
        Box::new(move |cc| Ok(Box::new(app::App::new(cc, launch, window)))),
    )
    .map_err(|e| anyhow::anyhow!("Cannot start native renderer: {e}"))
}

fn native_options(window: &window_state::WindowState) -> eframe::NativeOptions {
    let mut options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("Neptune")
            .with_app_id("rs.neptune.terminal")
            .with_icon(native_icon())
            .with_inner_size(window.inner_size)
            .with_maximized(window.maximized)
            .with_min_inner_size([640.0, 400.0])
            .with_transparent(true)
            .with_decorations(false),
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };
    if cfg!(target_os = "windows") {
        options.wgpu_options.wgpu_setup = windows_gpu_setup();
    }
    options
}

fn windows_gpu_setup() -> eframe::egui_wgpu::WgpuSetup {
    use eframe::{egui_wgpu::WgpuSetupCreateNew, wgpu};

    let mut setup = WgpuSetupCreateNew::without_display_handle();
    // HWND swapchains are opaque. DirectComposition preserves the alpha in
    // our painted corners; use DX12 so an opaque Vulkan surface cannot win.
    setup.instance_descriptor.backends = wgpu::Backends::DX12;
    setup
        .instance_descriptor
        .backend_options
        .dx12
        .presentation_system = wgpu::Dx12SwapchainKind::DxgiFromVisual;
    setup.into()
}

fn native_icon() -> eframe::egui::IconData {
    let mut rgba = vec![0; 128 * 128 * 4];
    let distance = |x: f32, y: f32, a: [f32; 2], b: [f32; 2]| {
        let dx = b[0] - a[0];
        let dy = b[1] - a[1];
        let t = (((x - a[0]) * dx + (y - a[1]) * dy) / (dx * dx + dy * dy)).clamp(0.0, 1.0);
        ((x - a[0] - t * dx).powi(2) + (y - a[1] - t * dy).powi(2)).sqrt()
    };
    for y in 0..128 {
        for x in 0..128 {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;
            let qx = (px - 64.0).abs() - 34.0;
            let qy = (py - 64.0).abs() - 34.0;
            let corner =
                (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt() + qx.max(qy).min(0.0) - 30.0;
            let alpha = (0.5 - corner).clamp(0.0, 1.0);
            let line = distance(px, py, [35.0, 37.0], [60.0, 64.0])
                .min(distance(px, py, [60.0, 64.0], [35.0, 91.0]))
                .min(distance(px, py, [73.0, 91.0], [95.0, 91.0]));
            let blend = (5.0 - line).clamp(0.0, 1.0);
            let offset = (y * 128 + x) * 4;
            for (channel, (base, ink)) in [(23.0, 185.0), (23.0, 172.0), (25.0, 242.0)]
                .into_iter()
                .enumerate()
            {
                rgba[offset + channel] = (base + (ink - base) * blend) as u8;
            }
            rgba[offset + 3] = (alpha * 255.0) as u8;
        }
    }
    eframe::egui::IconData {
        rgba,
        width: 128,
        height: 128,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_window_supports_transparent_corners() {
        let options = native_options(&window_state::WindowState::default());
        assert_eq!(options.viewport.transparent, Some(true));
        assert_eq!(options.viewport.decorations, Some(false));
    }

    #[test]
    fn windows_surface_supports_alpha_composition() {
        use eframe::wgpu::{Backends, Dx12SwapchainKind};

        let eframe::egui_wgpu::WgpuSetup::CreateNew(setup) = windows_gpu_setup() else {
            panic!("the desktop creates its GPU setup");
        };
        assert_eq!(setup.instance_descriptor.backends, Backends::DX12);
        assert_eq!(
            setup
                .instance_descriptor
                .backend_options
                .dx12
                .presentation_system,
            Dx12SwapchainKind::DxgiFromVisual
        );
    }
}
