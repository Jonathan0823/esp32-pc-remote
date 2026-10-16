use std::{
    net::IpAddr,
    path::PathBuf,
    sync::{Arc, Mutex, mpsc},
    time::Duration,
};

use eframe::egui;
use tokio::runtime::Handle;
use tray_icon::{
    Icon, TrayIconBuilder,
    menu::{Menu, MenuEvent, MenuItem},
};

use super::windows_runtime::{WindowsRuntime, acquire_single_instance};
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
    QuitComplete(bool),
}

#[derive(Default)]
struct UiSignals {
    context: Mutex<Option<egui::Context>>,
    sender: Mutex<Option<mpsc::Sender<UiEvent>>>,
}

impl UiSignals {
    fn wake(&self, event: UiEvent) {
        if let Ok(sender) = self.sender.lock() {
            if let Some(sender) = sender.as_ref() {
                let _ = sender.send(event);
            }
        }
        if let Ok(context) = self.context.lock() {
            if let Some(context) = context.as_ref() {
                context.request_repaint();
            }
        }
    }
}

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let Some(_instance) = acquire_single_instance()? else {
        return Ok(());
    };
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
    let handle = runtime.handle().clone();
    let (sender, receiver) = mpsc::channel();
    let signals = Arc::new(UiSignals::default());
    if let Ok(mut sender_slot) = signals.sender.lock() {
        *sender_slot = Some(sender);
    }

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("ESP32 Media Daemon")
            .with_visible(false)
            .with_inner_size([560.0, 520.0]),
        ..Default::default()
    };
    eframe::run_native(
        "ESP32 Media Daemon",
        options,
        Box::new(move |creation| {
            Ok(Box::new(SettingsApp::new(
                creation,
                signals,
                receiver,
                service,
                handle,
                startup_error,
            )))
        }),
    )?;
    Ok(())
}

fn create_tray(signals: Arc<UiSignals>) -> Result<tray_icon::TrayIcon, String> {
    let menu = Menu::new();
    let open = MenuItem::with_id("settings", "Settings", true, None);
    let toggle = MenuItem::with_id("listener", "Pause / resume listener", true, None);
    let quit = MenuItem::with_id("quit", "Quit", true, None);
    menu.append(&open).map_err(|error| error.to_string())?;
    menu.append(&toggle).map_err(|error| error.to_string())?;
    menu.append(&quit).map_err(|error| error.to_string())?;
    MenuEvent::set_event_handler(Some(move |event: MenuEvent| match event.id.0.as_str() {
        "settings" => signals.wake(UiEvent::OpenSettings),
        "listener" => signals.wake(UiEvent::ToggleListener),
        "quit" => signals.wake(UiEvent::Quit),
        _ => {}
    }));

    let icon = Icon::from_rgba(vec![0x35, 0x9f, 0xe8, 0xff].repeat(16 * 16), 16, 16)
        .map_err(|error| error.to_string())?;
    TrayIconBuilder::new()
        .with_tooltip("ESP32 Media Daemon")
        .with_icon(icon)
        .with_menu(Box::new(menu))
        .build()
        .map_err(|error| error.to_string())
}

struct SettingsApp {
    signals: Arc<UiSignals>,
    receiver: mpsc::Receiver<UiEvent>,
    service: Arc<WindowsRuntime>,
    handle: Handle,
    show_settings: bool,
    listener_enabled: bool,
    listener_pending: bool,
    actions_pending: bool,
    startup_pending: bool,
    quit_requested: bool,
    shutdown_pending: bool,
    tray_error: Option<String>,
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
    _tray: Option<tray_icon::TrayIcon>,
}

impl SettingsApp {
    fn new(
        creation: &eframe::CreationContext<'_>,
        signals: Arc<UiSignals>,
        receiver: mpsc::Receiver<UiEvent>,
        service: Arc<WindowsRuntime>,
        handle: Handle,
        startup_error: bool,
    ) -> Self {
        if let Ok(mut context_slot) = signals.context.lock() {
            *context_slot = Some(creation.egui_ctx.clone());
        }
        let config = service.config();
        let tray_result = create_tray(Arc::clone(&signals));
        let tray_error = tray_result.as_ref().err().cloned();
        let show_settings = tray_error.is_some();
        if show_settings {
            creation
                .egui_ctx
                .send_viewport_cmd(egui::ViewportCommand::Visible(true));
        }
        Self {
            signals,
            receiver,
            service,
            handle,
            show_settings,
            listener_enabled: config.listener_enabled,
            listener_pending: false,
            actions_pending: false,
            startup_pending: false,
            quit_requested: false,
            shutdown_pending: false,
            tray_error,
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
            _tray: tray_result.ok(),
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

impl eframe::App for SettingsApp {
    fn update(&mut self, context: &egui::Context, _frame: &mut eframe::Frame) {
        while let Ok(event) = self.receiver.try_recv() {
            match event {
                UiEvent::OpenSettings => {
                    self.show_settings = true;
                    context.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                }
                UiEvent::ToggleListener => self.request_listener(!self.listener_enabled),
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
                UiEvent::Quit => {
                    if !self.shutdown_pending {
                        self.shutdown_pending = true;
                        let service = Arc::clone(&self.service);
                        let signals = Arc::clone(&self.signals);
                        self.handle.spawn(async move {
                            signals.wake(UiEvent::QuitComplete(service.shutdown().await.is_ok()));
                        });
                    }
                }
                UiEvent::QuitComplete(success) => {
                    self.quit_requested = true;
                    if !success {
                        self.status = Some("Shutdown reported a listener error".into());
                    }
                    context.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
        }
        if context.input(|input| input.viewport().close_requested()) && !self.quit_requested {
            context.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            context.send_viewport_cmd(egui::ViewportCommand::Visible(false));
            self.show_settings = false;
        }
        if !self.show_settings {
            context.request_repaint_after(Duration::from_secs(3600));
            return;
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
            if let Some(error) = &self.tray_error {
                ui.colored_label(egui::Color32::RED, format!("Tray unavailable: {error}"));
            }
            if let Some(status) = &self.status {
                ui.label(status);
            }
            if ui.button("Close to tray").clicked() {
                self.show_settings = false;
                context.send_viewport_cmd(egui::ViewportCommand::Visible(false));
            }
            });
        });
    }
}
