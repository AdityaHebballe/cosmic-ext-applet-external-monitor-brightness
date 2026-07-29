use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::config::{self, Config, MonitorConfig};
use crate::monitor;
use crate::monitor::{DisplayId, EventToSub, MonitorInfo, ScreenBrightness};
use crate::osd;
use crate::shortcut;
use anyhow::anyhow;
use cosmic::Element;
use cosmic::app::{Core, Task};
use cosmic::cosmic_config::Config as CosmicConfig;
use cosmic::cosmic_config::CosmicConfigEntry;
use cosmic::cosmic_theme::{THEME_MODE_ID, ThemeMode};
use cosmic::iced::core::window;
use cosmic::iced::window::Id;
use cosmic::iced::{Limits, Subscription};
use cosmic::widget::Space;
use tokio::sync::watch::Sender;

pub const APPID: &str = "io.github.cosmic_utils.cosmic-ext-applet-external-monitor-brightness";

#[derive(Debug, Clone)]
pub struct MonitorState {
    pub name: String,
    /// Between 0 and 1
    pub slider_brightness: f32,
    pub settings_expanded: bool,
}

pub fn get_mapped_brightness(slider_brightness: f32, gamma: f32) -> u16 {
    (slider_brightness.powf(gamma) * 100.0).round() as u16
}

pub fn get_slider_brightness(brightness: u16, gamma: f32) -> f32 {
    (brightness as f32 / 100.0).powf(1.0 / gamma)
}

impl MonitorState {
    pub fn get_mapped_brightness(&self, gamma: f32) -> u16 {
        get_mapped_brightness(self.slider_brightness, gamma)
    }

    pub fn set_slider_brightness(&mut self, brightness: u16, gamma: f32) {
        self.slider_brightness = get_slider_brightness(brightness, gamma)
    }
}

#[derive(Debug, Clone)]
struct Popup {
    pub kind: PopupKind,
    pub id: window::Id,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum PopupKind {
    Popup,
    QuickSettings,
}

fn now() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis()
}

impl AppState {
    fn toggle_popup(&mut self, kind: PopupKind) -> Task<AppMsg> {
        match &self.popup {
            Some(popup) => {
                if popup.kind == kind {
                    self.close_popup()
                } else {
                    Task::batch(vec![self.close_popup(), self.open_popup(kind)])
                }
            }
            None => self.open_popup(kind),
        }
    }

    fn close_popup(&mut self) -> Task<AppMsg> {
        for mon in self.monitors.values_mut() {
            mon.settings_expanded = false;
        }

        if let Some(popup) = self.popup.take() {
            self.last_quit = Some((now(), popup.kind));

            // info!("destroy {:?}", popup.id);
            cosmic::surface::surface_task(cosmic::surface::action::destroy_popup(popup.id))
        } else {
            Task::none()
        }
    }

    fn open_popup(&mut self, kind: PopupKind) -> Task<AppMsg> {
        // handle the case where the popup was closed by clicking the icon
        if self
            .last_quit
            .map(|(t, k)| (now() - t) < 200 && k == kind)
            .unwrap_or(false)
        {
            return Task::none();
        }

        let new_id = Id::unique();
        // info!("will create {:?}", new_id);

        let popup = Popup { kind, id: new_id };
        self.popup.replace(popup);

        match kind {
            PopupKind::Popup => {
                self.send(EventToSub::Refresh);
                cosmic::surface::surface_task(cosmic::surface::action::app_popup(
                    |_| Default::default(),
                    move |app: &mut Self| {
                        let mut popup_settings = app.core.applet.get_popup_settings(
                            app.core.main_window_id().unwrap(),
                            new_id,
                            None,
                            None,
                            None,
                        );
                        popup_settings.positioner.size_limits = Limits::NONE
                            .min_width(300.0)
                            .max_width(400.0)
                            .min_height(200.0)
                            .max_height(500.0);
                        popup_settings
                    },
                    None,
                ))
            }
            PopupKind::QuickSettings => {
                cosmic::surface::surface_task(cosmic::surface::action::app_popup(
                    |_| Default::default(),
                    move |app: &mut Self| {
                        let mut popup_settings = app.core.applet.get_popup_settings(
                            app.core.main_window_id().unwrap(),
                            new_id,
                            None,
                            None,
                            None,
                        );
                        popup_settings.positioner.size_limits = Limits::NONE
                            .min_width(200.0)
                            .max_width(250.0)
                            .min_height(200.0)
                            .max_height(550.0);
                        popup_settings
                    },
                    None,
                ))
            }
        }
    }
}

pub struct AppState {
    pub core: Core,
    popup: Option<Popup>,
    pub monitors: HashMap<DisplayId, MonitorState>,
    pub theme_mode_config: ThemeMode,
    sender: Option<Sender<EventToSub>>,
    pub config: Config,
    config_handler: CosmicConfig,
    last_quit: Option<(u128, PopupKind)>,
    osd: Option<osd::State>,
}

#[derive(Clone, Debug)]
pub enum AppMsg {
    TogglePopup,
    ToggleQuickSettings,
    ClosePopup,
    OsdRequested,
    Osd(osd::Msg),

    ConfigChanged(Config),
    ThemeModeConfigChanged(ThemeMode),
    SetDarkMode(bool),

    SetScreenBrightness(DisplayId, f32),
    ToggleMinMaxBrightness(DisplayId),
    ChangeGlobalBrightness {
        delta: f32,
    },
    ToggleMonSettings(DisplayId),
    SetMonGammaMap(DisplayId, f32),

    /// Send from the subscription
    SubscriptionReady((HashMap<DisplayId, MonitorInfo>, Sender<EventToSub>)),
    /// Send from the subscription
    BrightnessWasUpdated(DisplayId, ScreenBrightness),
    Refresh,
}

impl AppState {
    pub fn send(&self, e: EventToSub) {
        if let Some(sender) = &self.sender {
            if let Err(err) = sender.send(e) {
                debug!(?err, "monitor subscription is no longer listening");
            }
        }
    }

    fn update_monitor_config(&mut self, id: String, f: impl Fn(&mut MonitorConfig)) {
        let mut monitors = std::mem::take(&mut self.config.monitors);

        let monitor = monitors.entry(id).or_insert(MonitorConfig::new());
        f(monitor);

        if let Err(e) = self.config.set_monitors(&self.config_handler, monitors) {
            error!("can't write config: {e}");
        }
    }

    fn show_osd(&mut self) -> Task<AppMsg> {
        let brightness = if self.monitors.is_empty() {
            0.0
        } else {
            self.monitors
                .iter()
                .map(|(id, monitor)| {
                    monitor.get_mapped_brightness(self.config.get_gamma_map(id)) as f32 / 100.0
                })
                .sum::<f32>()
                / self.monitors.len() as f32
        };

        self.show_local_osd(brightness)
    }

    fn show_local_osd(&mut self, brightness: f32) -> Task<AppMsg> {
        if let Some(state) = &mut self.osd {
            state
                .replace_brightness(brightness)
                .map(|x| cosmic::Action::App(AppMsg::Osd(x)))
        } else {
            let (state, cmd) = osd::State::new(brightness);
            self.osd = Some(state);
            cmd
        }
    }
}

impl cosmic::Application for AppState {
    type Executor = cosmic::SingleThreadExecutor;
    type Flags = (Option<CosmicConfig>, Config);
    type Message = AppMsg;
    const APP_ID: &'static str = APPID;

    fn core(&self) -> &Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    fn init(core: Core, flags: Self::Flags) -> (Self, Task<Self::Message>) {
        let window = AppState {
            core,
            config_handler: flags.0.expect("need to be able to write config"),
            config: flags.1,
            popup: None,
            monitors: HashMap::new(),
            theme_mode_config: ThemeMode::default(),
            sender: None,
            last_quit: None,
            osd: None,
        };

        (window, Task::none())
    }

    fn on_close_requested(&self, id: window::Id) -> Option<AppMsg> {
        info!("on_close_requested");

        if let Some(popup) = &self.popup
            && popup.id == id
        {
            return Some(AppMsg::ClosePopup);
        }
        None
    }

    fn update(&mut self, message: Self::Message) -> Task<Self::Message> {
        debug!("{:?}", message);

        match message {
            AppMsg::TogglePopup => {
                return self.toggle_popup(PopupKind::Popup);
            }
            AppMsg::ToggleQuickSettings => return self.toggle_popup(PopupKind::QuickSettings),
            AppMsg::ClosePopup => return self.close_popup(),
            AppMsg::OsdRequested => {}
            AppMsg::Osd(msg) => {
                if let Some(state) = self.osd.take() {
                    let (state, cmd) = state.update(msg);
                    self.osd = state;
                    return cmd.map(|x| cosmic::Action::App(AppMsg::Osd(x)));
                }
            }
            AppMsg::SetScreenBrightness(id, slider_brightness) => {
                if let Some(monitor) = self.monitors.get_mut(&id) {
                    monitor.slider_brightness = slider_brightness;
                    let gamma = self.config.get_gamma_map(&id);
                    let b = monitor.get_mapped_brightness(gamma);
                    self.send(EventToSub::Set(id, b));
                    return self.show_local_osd(b as f32 / 100.0);
                }
            }
            AppMsg::ChangeGlobalBrightness { delta } => {
                let mut vec = Vec::with_capacity(self.monitors.len());

                for (id, monitor) in self.monitors.iter_mut() {
                    monitor.slider_brightness = (monitor.slider_brightness + delta).clamp(0.0, 1.0);

                    let gamma = self.config.get_gamma_map(id);

                    let b = monitor.get_mapped_brightness(gamma);

                    vec.push(EventToSub::Set(id.clone(), b));
                }

                for e in vec {
                    self.send(e);
                }

                return self.show_osd();
            }
            AppMsg::ToggleMinMaxBrightness(id) => {
                if let Some(monitor) = self.monitors.get_mut(&id) {
                    let new_val = match monitor.slider_brightness {
                        x if x < 0.5 => 100,
                        _ => 0,
                    };
                    monitor.slider_brightness = new_val as f32 / 100.0;
                    self.send(EventToSub::Set(id, new_val));
                }
            }
            AppMsg::ThemeModeConfigChanged(config) => {
                self.theme_mode_config = config;
            }
            AppMsg::SetDarkMode(dark) => {
                #[allow(dead_code)]
                fn set_theme_mode(mode: &ThemeMode) -> anyhow::Result<()> {
                    let home_dir = dirs::home_dir().ok_or(anyhow!("no home dir"))?;

                    let helper = cosmic::cosmic_config::Config::with_custom_path(
                        THEME_MODE_ID,
                        ThemeMode::VERSION,
                        home_dir.join(".config"),
                    )?;

                    mode.write_entry(&helper)?;

                    Ok(())
                }

                fn set_theme_mode2(mode: &ThemeMode) -> anyhow::Result<()> {
                    let helper = ThemeMode::config()?;
                    mode.write_entry(&helper)?;
                    Ok(())
                }

                self.theme_mode_config.is_dark = dark;

                if let Err(e) = set_theme_mode2(&self.theme_mode_config) {
                    error!("can't write theme mode {e}");
                }
            }
            AppMsg::SubscriptionReady((monitors, sender)) => {
                self.monitors = monitors
                    .into_iter()
                    .map(|(id, m)| {
                        let gamma_map = self.config.get_gamma_map(&id);
                        (
                            id,
                            MonitorState {
                                name: m.name,
                                slider_brightness: get_slider_brightness(m.brightness, gamma_map),
                                settings_expanded: false,
                            },
                        )
                    })
                    .collect();

                self.sender.replace(sender);
            }
            AppMsg::BrightnessWasUpdated(id, brightness) => {
                if let Some(monitor) = self.monitors.get_mut(&id) {
                    monitor.set_slider_brightness(brightness, self.config.get_gamma_map(&id));
                }
            }
            AppMsg::SetMonGammaMap(id, gamma) => {
                if let Some(monitor) = self.monitors.get(&id) {
                    let b = monitor.get_mapped_brightness(gamma);
                    self.send(EventToSub::Set(id.clone(), b));
                }

                self.update_monitor_config(id, |monitor| {
                    monitor.gamma_map = gamma;
                });
            }
            AppMsg::ToggleMonSettings(id) => {
                if let Some(mon) = self.monitors.get_mut(&id) {
                    mon.settings_expanded = !mon.settings_expanded;
                }
            }
            AppMsg::ConfigChanged(config) => self.config = config,
            AppMsg::Refresh => {
                self.send(EventToSub::Refresh);
            }
        }
        Task::none()
    }

    fn view(&self) -> Element<'_, Self::Message> {
        self.applet_button_view()
    }

    fn view_window(&self, id: Id) -> Element<'_, Self::Message> {
        if let Some(osd) = &self.osd
            && osd.id() == id
        {
            return osd.view().map(AppMsg::Osd);
        }

        let Some(popup) = &self.popup else {
            return Space::new().into();
        };

        let view = match &popup.kind {
            PopupKind::Popup => self.popup_view(),
            PopupKind::QuickSettings => self.quick_settings_view(),
        };

        self.core.applet.popup_container(view).into()
    }

    fn style(&self) -> Option<cosmic::iced::theme::Style> {
        Some(cosmic::applet::style())
    }

    fn subscription(&self) -> Subscription<Self::Message> {
        Subscription::batch(vec![
            self.core
                .watch_config(THEME_MODE_ID)
                .map(|u| AppMsg::ThemeModeConfigChanged(u.config)),
            Subscription::run(monitor::sub),
            Subscription::run(shortcut::sub),
            config::sub(),
            // Subscription::run(refresh_sub),
        ])
    }
}
