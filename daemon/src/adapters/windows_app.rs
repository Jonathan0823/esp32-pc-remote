use std::{
    net::IpAddr,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
};

use eframe::egui;
use tokio::runtime::Handle;
use tray_icon::{
    Icon, MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent,
    menu::{Menu, MenuEvent, MenuItem},
};
use winit::event_loop::EventLoop;

use super::{
    windows_host,
    windows_runtime::{WindowsRuntime, acquire_single_instance},
};
use crate::domain::{ActionId, ConfiguredAction, CustomAction};

#[derive(Clone, Copy)]
enum UiEvent {
    OpenSettings,
    ToggleListener,
    ListenerState(bool),
    ConfigState(bool),
    ActionTestResult(bool),
    ActionsSaved(bool),
    StartupState(bool),
    CredentialsRotated(bool),
    Quit,
}

struct UiSignals {
    context: Mutex<Option<egui::Context>>,
    sender: mpsc::Sender<UiEvent>,
    receiver: Mutex<mpsc::Receiver<UiEvent>>,
    updates_sender: mpsc::Sender<UiEvent>,
    updates_receiver: Mutex<mpsc::Receiver<UiEvent>>,
    thread_id: u32,
    settings_initialized: AtomicBool,
}

impl UiSignals {
    fn new() -> Self {
        let (sender, receiver) = mpsc::channel();
        let (updates_sender, updates_receiver) = mpsc::channel();
        Self {
            context: Mutex::new(None),
            sender,
            receiver: Mutex::new(receiver),
            updates_sender,
            updates_receiver: Mutex::new(updates_receiver),
            thread_id: windows_host::current_thread_id(),
            settings_initialized: AtomicBool::new(false),
        }
    }

    fn next_event(&self) -> Option<UiEvent> {
        self.receiver.lock().ok()?.try_recv().ok()
    }

    fn next_update(&self) -> Option<UiEvent> {
        self.updates_receiver.lock().ok()?.try_recv().ok()
    }

    fn publish_update(&self, event: UiEvent) {
        let context = self.context.lock().ok().and_then(|context| context.clone());
        if let Some(context) = context {
            let _ = self.updates_sender.send(event);
            context.request_repaint();
        }
    }

    fn menu_event(&self, id: &str) {
        self.wake(match id {
            "settings" => UiEvent::OpenSettings,
            "listener" => UiEvent::ToggleListener,
            "quit" => UiEvent::Quit,
            _ => return,
        });
    }

    fn tray_event(&self, event: TrayIconEvent) {
        if matches!(
            event,
            TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            }
        ) {
            self.wake(UiEvent::OpenSettings);
        }
    }

    fn wake(&self, event: UiEvent) {
        let _ = self.sender.send(event);
        windows_host::wake_thread(self.thread_id);
    }
}

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let Some(_instance) = acquire_single_instance()? else {
        return Ok(());
    };
    // Establish DPI awareness before creating tray/host HWNDs; this creates no renderer.
    let mut event_loop = EventLoop::with_user_event().build()?;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    let service = WindowsRuntime::load()?;
    let startup_error =
        service.config().listener_enabled && runtime.block_on(service.set_listener(true)).is_err();
    if startup_error {
        tracing::warn!("saved listener could not be started; it remains paused");
    }
    let _entered = runtime.enter();
    let signals = Arc::new(UiSignals::new());
    let _host = windows_host::MessageHost::new()?;
    let _tray = create_tray(Arc::clone(&signals)).map_err(|error| {
        windows_host::show_error(&format!("Could not create the system tray: {error}"));
        std::io::Error::other(error)
    })?;

    let mut settings: Option<eframe::EframeWinitApplication<'static>> = None;
    let mut quitting = false;
    loop {
        if windows_host::take_open_request() {
            signals.wake(UiEvent::OpenSettings);
        }
        while let Some(event) = signals.next_event() {
            match event {
                UiEvent::OpenSettings if !quitting => {
                    if settings.is_some() {
                        windows_host::reveal_settings();
                    } else {
                        settings = Some(open_settings(
                            Arc::clone(&signals),
                            Arc::clone(&service),
                            runtime.handle().clone(),
                            startup_error,
                            &event_loop,
                        ));
                    }
                }
                UiEvent::ToggleListener if !quitting => {
                    let service = Arc::clone(&service);
                    let signals = Arc::clone(&signals);
                    runtime.spawn(async move {
                        let enabled = !service.config().listener_enabled;
                        let _ = service.set_listener(enabled).await;
                        signals.wake(UiEvent::ListenerState(service.config().listener_enabled));
                    });
                }
                UiEvent::Quit => quitting = true,
                UiEvent::OpenSettings | UiEvent::ToggleListener => {}
                update => signals.publish_update(update),
            }
        }
        if quitting {
            if settings.is_none() {
                break;
            }
            let context = signals
                .context
                .lock()
                .ok()
                .and_then(|context| context.clone());
            if let Some(context) = context {
                windows_host::reveal_settings();
                context.send_viewport_cmd(egui::ViewportCommand::Close);
                context.request_repaint();
            }
        }
        if let Some(settings_app) = &mut settings {
            // Block on OS/repaint events, then dispatch tray commands even if Settings is minimized.
            if let eframe::EframePumpStatus::Exit(code) =
                settings_app.pump_eframe_app(&mut event_loop, None)
            {
                settings = None;
                if !signals.settings_initialized.swap(false, Ordering::AcqRel) || code != 0 {
                    windows_host::show_error(&format!(
                        "Settings could not run (exit code {code}); check graphics initialization."
                    ));
                }
            }
        } else if !windows_host::dispatch_message()? {
            break;
        }
    }
    runtime.block_on(service.shutdown())?;
    Ok(())
}

fn open_settings(
    signals: Arc<UiSignals>,
    service: Arc<WindowsRuntime>,
    handle: Handle,
    startup_error: bool,
    event_loop: &EventLoop<eframe::UserEvent>,
) -> eframe::EframeWinitApplication<'static> {
    let options = eframe::NativeOptions {
        run_and_return: true,
        viewport: egui::ViewportBuilder::default()
            .with_title("ESP32 Media Daemon")
            .with_inner_size([560.0, 520.0]),
        ..Default::default()
    };
    eframe::create_native(
        "ESP32 Media Daemon",
        options,
        Box::new(move |creation| {
            Ok(Box::new(SettingsApp::new(
                creation,
                signals,
                service,
                handle,
                startup_error,
            )))
        }),
        event_loop,
    )
}

fn create_tray(signals: Arc<UiSignals>) -> Result<tray_icon::TrayIcon, String> {
    let menu = Menu::new();
    let open = MenuItem::with_id("settings", "Settings", true, None);
    let toggle = MenuItem::with_id("listener", "Pause / resume listener", true, None);
    let quit = MenuItem::with_id("quit", "Quit", true, None);
    menu.append(&open).map_err(|error| error.to_string())?;
    menu.append(&toggle).map_err(|error| error.to_string())?;
    menu.append(&quit).map_err(|error| error.to_string())?;
    let menu_signals = Arc::clone(&signals);
    MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
        menu_signals.menu_event(event.id.0.as_str());
    }));
    TrayIconEvent::set_event_handler(Some(move |event: TrayIconEvent| {
        signals.tray_event(event);
    }));

    let icon = Icon::from_rgba([0x35, 0x9f, 0xe8, 0xff].repeat(16 * 16), 16, 16)
        .map_err(|error| error.to_string())?;
    TrayIconBuilder::new()
        .with_tooltip("ESP32 Media Daemon")
        .with_icon(icon)
        .with_menu(Box::new(menu))
        .with_menu_on_left_click(false)
        .build()
        .map_err(|error| error.to_string())
}

struct SettingsApp {
    signals: Arc<UiSignals>,
    service: Arc<WindowsRuntime>,
    handle: Handle,
    listener_enabled: bool,
    listener_pending: bool,
    actions_pending: bool,
    startup_pending: bool,
    status: Option<String>,
    bind_address: String,
    port: String,
    pairing_token: Option<String>,
    start_after_login: bool,
    action_id: String,
    action_name: String,
    action_path: String,
    interpreter_path: String,
    action_args_json: String,
    action_kind: usize,
    confirm_action: Option<String>,
    confirm_rotation: bool,
}

impl SettingsApp {
    fn new(
        creation: &eframe::CreationContext<'_>,
        signals: Arc<UiSignals>,
        service: Arc<WindowsRuntime>,
        handle: Handle,
        startup_error: bool,
    ) -> Self {
        if let Ok(mut context_slot) = signals.context.lock() {
            *context_slot = Some(creation.egui_ctx.clone());
        }
        signals.settings_initialized.store(true, Ordering::Release);
        let config = service.config();
        Self {
            signals,
            service,
            handle,
            listener_enabled: config.listener_enabled,
            listener_pending: false,
            actions_pending: false,
            startup_pending: false,
            status: startup_error
                .then(|| "Saved listener could not start; check its address and port".into()),
            bind_address: config.bind_address.to_string(),
            port: config.port.to_string(),
            pairing_token: None,
            start_after_login: config.start_after_login,
            action_id: String::new(),
            action_name: String::new(),
            action_path: String::new(),
            interpreter_path: String::new(),
            action_args_json: "[]".into(),
            action_kind: 0,
            confirm_action: None,
            confirm_rotation: false,
        }
    }

    fn request_listener(&mut self, enabled: bool) {
        if self.listener_pending {
            return;
        }
        self.listener_pending = true;
        self.listener_enabled = enabled;
        let service = Arc::clone(&self.service);
        let signals = Arc::clone(&self.signals);
        self.handle.spawn(async move {
            let result = service.set_listener(enabled).await;
            let actual = if result.is_ok() {
                enabled
            } else {
                service.config().listener_enabled
            };
            signals.wake(UiEvent::ListenerState(actual));
        });
    }

    fn save_actions(&mut self, actions: Vec<CustomAction>) {
        if self.actions_pending {
            return;
        }
        self.actions_pending = true;
        let service = Arc::clone(&self.service);
        let signals = Arc::clone(&self.signals);
        self.handle.spawn(async move {
            signals.wake(UiEvent::ActionsSaved(
                service.save_actions(actions).await.is_ok(),
            ));
        });
    }

    fn set_startup(&mut self, enabled: bool) {
        if self.startup_pending {
            return;
        }
        self.startup_pending = true;
        let service = Arc::clone(&self.service);
        let signals = Arc::clone(&self.signals);
        self.handle.spawn(async move {
            signals.wake(UiEvent::StartupState(
                service.set_start_after_login(enabled).await.is_ok(),
            ));
        });
    }

    fn add_action(&mut self) {
        let Ok(id) = ActionId::parse(self.action_id.trim()) else {
            self.status = Some("Action ID must match [a-z][a-z0-9_]{0,63}".into());
            return;
        };
        let Ok(args) = serde_json::from_str::<Vec<String>>(&self.action_args_json) else {
            self.status = Some("Arguments must be a JSON string array".into());
            return;
        };
        let path = PathBuf::from(&self.action_path);
        let action = match self.action_kind {
            0 => ConfiguredAction::Launch {
                program: path,
                args,
                working_dir: None,
            },
            1 => ConfiguredAction::Batch {
                script: path,
                args,
                working_dir: None,
            },
            _ => ConfiguredAction::PowerShell {
                interpreter: PathBuf::from(&self.interpreter_path),
                script: path,
                args,
                working_dir: None,
            },
        };
        let mut actions = self.service.config().actions;
        actions.push(CustomAction {
            id,
            name: self.action_name.trim().to_owned(),
            enabled: true,
            action,
        });
        let mut candidate = self.service.config();
        candidate.actions = actions.clone();
        if candidate.validate().is_err() {
            self.status = Some("Invalid action".into());
            return;
        }
        self.action_id.clear();
        self.action_name.clear();
        self.action_path.clear();
        self.save_actions(actions);
    }
}

impl Drop for SettingsApp {
    fn drop(&mut self) {
        if let Ok(mut context) = self.signals.context.lock() {
            *context = None;
        }
        while self.signals.next_update().is_some() {}
    }
}

impl eframe::App for SettingsApp {
    fn update(&mut self, context: &egui::Context, _frame: &mut eframe::Frame) {
        while let Some(event) = self.signals.next_update() {
            match event {
                UiEvent::ListenerState(enabled) => {
                    self.listener_pending = false;
                    self.listener_enabled = enabled;
                    self.status = Some(
                        if enabled {
                            "Listener enabled"
                        } else {
                            "Listener paused"
                        }
                        .into(),
                    );
                }
                UiEvent::ConfigState(success) => {
                    self.status = Some(
                        if success {
                            "Configuration applied"
                        } else {
                            "Configuration was not applied"
                        }
                        .into(),
                    );
                }
                UiEvent::CredentialsRotated(success) => {
                    self.pairing_token = None;
                    self.listener_enabled = self.service.config().listener_enabled;
                    self.status = Some(
                        if success {
                            "Token and TLS trust rotated; pair clients again"
                        } else {
                            "Credential rotation failed"
                        }
                        .into(),
                    );
                }
                UiEvent::ActionsSaved(success) => {
                    self.actions_pending = false;
                    self.status = Some(
                        if success {
                            "Action configuration saved"
                        } else {
                            "Invalid action or configuration save failed"
                        }
                        .into(),
                    );
                }
                UiEvent::StartupState(success) => {
                    self.startup_pending = false;
                    self.start_after_login = self.service.config().start_after_login;
                    self.status = Some(
                        if success {
                            "Startup preference saved"
                        } else {
                            "Could not update startup preference"
                        }
                        .into(),
                    );
                }
                UiEvent::ActionTestResult(success) => {
                    self.status = Some(
                        if success {
                            "Test was dispatched"
                        } else {
                            "Test failed"
                        }
                        .into(),
                    );
                }
                UiEvent::OpenSettings | UiEvent::ToggleListener | UiEvent::Quit => {}
            }
        }
        egui::CentralPanel::default().show(context, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.heading("ESP32 Media Daemon");
            let listener_changed = ui
                .add_enabled(
                    !self.listener_pending,
                    egui::Checkbox::new(&mut self.listener_enabled, "HTTPS listener enabled"),
                )
                .changed();
            if listener_changed {
                self.request_listener(self.listener_enabled);
            }
            ui.horizontal(|ui| {
                ui.label("Bind IP");
                ui.add_enabled(
                    !self.listener_enabled,
                    egui::TextEdit::singleline(&mut self.bind_address),
                );
                ui.label("Port");
                ui.add_enabled(
                    !self.listener_enabled,
                    egui::TextEdit::singleline(&mut self.port).desired_width(70.0),
                );
                if ui
                    .add_enabled(!self.listener_enabled, egui::Button::new("Apply"))
                    .clicked()
                {
                    let parsed = self.bind_address.parse::<IpAddr>();
                    let port = self.port.parse::<u16>();
                    match (parsed, port) {
                        (Ok(address), Ok(port)) => {
                            let service = Arc::clone(&self.service);
                            let signals = Arc::clone(&self.signals);
                            self.handle.spawn(async move {
                                let success = service.reconfigure(address, port).await.is_ok();
                                signals.wake(UiEvent::ConfigState(success));
                            });
                        }
                        _ => self.status = Some("Enter a valid IP address and port".into()),
                    }
                }
            });
            ui.separator();
            ui.label("Pairing credentials authorize configured actions.");
            if ui.button("Show pairing token").clicked() {
                self.pairing_token = self.service.token_for_pairing().ok();
            }
            if let Some(token) = &self.pairing_token {
                ui.monospace(token);
                if ui.button("Copy token").clicked() {
                    context.copy_text(token.clone());
                }
            }
            if ui.button("Rotate token and TLS trust…").clicked() {
                self.confirm_rotation = true;
            }
            if ui.button("Export trusted CA PEM").clicked() {
                self.status = Some(match self.service.export_ca() {
                    Ok(path) => format!("CA exported to {}", path.display()),
                    Err(_) => "Could not export the trusted CA".into(),
                });
            }
            if ui
                .add_enabled(
                    !self.startup_pending,
                    egui::Checkbox::new(&mut self.start_after_login, "Start after login"),
                )
                .changed()
            {
                self.set_startup(self.start_after_login);
            }
            ui.label("Changing the bind IP regenerates the CA; pair clients again.");
            ui.separator();
            ui.heading("Configured actions");
            ui.horizontal(|ui| {
                ui.label("ID");
                ui.text_edit_singleline(&mut self.action_id);
                ui.label("Name");
                ui.text_edit_singleline(&mut self.action_name);
            });
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.action_kind, 0, "Application");
                ui.selectable_value(&mut self.action_kind, 1, "Batch");
                ui.selectable_value(&mut self.action_kind, 2, "PowerShell");
            });
            ui.text_edit_singleline(&mut self.action_path);
            if self.action_kind == 2 {
                ui.text_edit_singleline(&mut self.interpreter_path);
                ui.label("PowerShell 5.1 powershell.exe or installed PowerShell 7 pwsh.exe");
            }
            ui.label("Arguments as a JSON string array, e.g. [\"--play\"]");
            ui.text_edit_singleline(&mut self.action_args_json);
            if ui
                .add_enabled(!self.actions_pending, egui::Button::new("Add action"))
                .clicked()
            {
                self.add_action();
            }
            let actions = self.service.config().actions;
            for action in actions {
                ui.horizontal(|ui| {
                    let mut enabled = action.enabled;
                    if ui
                        .add_enabled(!self.actions_pending, egui::Checkbox::new(&mut enabled, ""))
                        .changed()
                    {
                        let mut updated = self.service.config().actions;
                        if let Some(item) = updated.iter_mut().find(|item| item.id == action.id) {
                            item.enabled = enabled;
                        }
                        self.save_actions(updated);
                    }
                    ui.label(format!("{} ({})", action.name, action.id.as_str()));
                    if ui.button("Test…").clicked() {
                        self.confirm_action = Some(action.id.as_str().to_string());
                    }
                    if ui
                        .add_enabled(!self.actions_pending, egui::Button::new("Remove"))
                        .clicked()
                    {
                        let mut remaining = self.service.config().actions;
                        remaining.retain(|existing| existing.id != action.id);
                        self.save_actions(remaining);
                    }
                });
            }
            if let Some(action_id) = self.confirm_action.clone() {
                egui::Window::new("Confirm action test")
                    .collapsible(false)
                    .show(context, |ui| {
                        ui.label(
                            "This runs the configured action with your Windows user permissions.",
                        );
                        ui.horizontal(|ui| {
                            if ui.button("Run test").clicked() {
                                self.confirm_action = None;
                                if let Ok(id) = ActionId::parse(action_id) {
                                    let service = Arc::clone(&self.service);
                                    let signals = Arc::clone(&self.signals);
                                    self.handle.spawn(async move {
                                        signals.wake(UiEvent::ActionTestResult(
                                            service.test_action(&id).await.is_ok(),
                                        ));
                                    });
                                }
                            }
                            if ui.button("Cancel").clicked() {
                                self.confirm_action = None;
                            }
                        });
                    });
            }
            if self.confirm_rotation {
                egui::Window::new("Confirm credential rotation")
                    .collapsible(false)
                    .show(context, |ui| {
                        ui.label("This pauses the listener and invalidates the current token and trusted CA. Existing clients must be paired again.");
                        ui.horizontal(|ui| {
                            if ui.button("Rotate credentials").clicked() {
                                self.confirm_rotation = false;
                                let service = Arc::clone(&self.service);
                                let signals = Arc::clone(&self.signals);
                                self.handle.spawn(async move {
                                    signals.wake(UiEvent::CredentialsRotated(service.rotate_credentials().await.is_ok()));
                                });
                            }
                            if ui.button("Cancel").clicked() {
                                self.confirm_rotation = false;
                            }
                        });
                    });
            }
            egui::CollapsingHeader::new("Recent diagnostics")
                .default_open(false)
                .show(ui, |ui| {
                    for event in self.service.diagnostics().iter().rev().take(10) {
                        ui.small(format!(
                            "{} | request {:?} | action {:?}",
                            event.category, event.request_id, event.action_id
                        ));
                    }
                });
            if let Some(status) = &self.status {
                ui.label(status);
            }
            if ui.button("Close to tray").clicked() {
                context.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            });
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn menu_commands_are_delivered_without_a_settings_context() {
        let signals = UiSignals::new();
        signals.menu_event("settings");
        signals.menu_event("listener");
        signals.menu_event("quit");
        signals.menu_event("unknown");
        assert!(matches!(signals.next_event(), Some(UiEvent::OpenSettings)));
        assert!(matches!(
            signals.next_event(),
            Some(UiEvent::ToggleListener)
        ));
        assert!(matches!(signals.next_event(), Some(UiEvent::Quit)));
        assert!(signals.next_event().is_none());
        assert!(signals.context.lock().unwrap().is_none());
    }

    #[test]
    fn ui_updates_do_not_consume_tray_commands() {
        let signals = UiSignals::new();
        *signals.context.lock().unwrap() = Some(egui::Context::default());
        signals.menu_event("quit");
        signals.publish_update(UiEvent::ListenerState(true));
        assert!(matches!(signals.next_event(), Some(UiEvent::Quit)));
        assert!(matches!(
            signals.next_update(),
            Some(UiEvent::ListenerState(true))
        ));
        assert!(signals.next_update().is_none());
        *signals.context.lock().unwrap() = None;
        signals.publish_update(UiEvent::ListenerState(false));
        assert!(signals.next_update().is_none());
    }

    #[test]
    fn only_left_button_release_opens_settings() {
        let signals = UiSignals::new();
        for (button, button_state) in [
            (MouseButton::Left, MouseButtonState::Down),
            (MouseButton::Right, MouseButtonState::Up),
        ] {
            signals.tray_event(TrayIconEvent::Click {
                id: "test".into(),
                position: Default::default(),
                rect: Default::default(),
                button,
                button_state,
            });
            assert!(signals.next_event().is_none());
        }
        signals.tray_event(TrayIconEvent::Click {
            id: "test".into(),
            position: Default::default(),
            rect: Default::default(),
            button: MouseButton::Left,
            button_state: MouseButtonState::Up,
        });
        assert!(matches!(signals.next_event(), Some(UiEvent::OpenSettings)));
        assert!(signals.context.lock().unwrap().is_none());
    }
}
