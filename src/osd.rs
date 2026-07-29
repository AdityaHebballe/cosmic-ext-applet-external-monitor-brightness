use cosmic::iced::futures::future::{AbortHandle, Aborted, abortable};
use cosmic::iced::platform_specific::shell::commands::layer_surface::{
    Anchor, KeyboardInteractivity, Layer, destroy_layer_surface,
};
use cosmic::iced::runtime::platform_specific::wayland::layer_surface::{
    IcedMargin, IcedOutput, SctkLayerSurfaceSettings,
};
use cosmic::iced::window::Id as SurfaceId;
use cosmic::iced::{self, Alignment, Border, Length};
use cosmic::surface::action::{LiveSettings, simple_layer_shell};
use cosmic::{Apply, Element, Task, widget};
use std::sync::LazyLock;
use std::time::Duration;

use crate::app::AppMsg;

static OSD_ID: LazyLock<widget::Id> = LazyLock::new(|| widget::Id::new("external-brightness-osd"));

#[derive(Clone, Debug)]
pub enum Msg {
    Ignore,
    Close,
}

#[derive(Debug)]
pub struct State {
    id: SurfaceId,
    brightness: f32,
    timer_abort: AbortHandle,
}

fn percent(brightness: f32) -> u32 {
    let mut p = (brightness * 100.0).round() as i32;
    if p <= 0 && brightness >= 0.0 {
        p = 1; // never show 0%
    }
    if p > 100 {
        p = 100;
    }
    p as u32
}

fn close_timer() -> (Task<Msg>, AbortHandle) {
    let (future, timer_abort) = abortable(async {
        tokio::time::sleep(Duration::from_secs(3)).await;
    });
    let command = cosmic::task::future(async move {
        match future.await {
            Ok(_) => Msg::Close,
            Err(Aborted) => Msg::Ignore,
        }
    });
    (command, timer_abort)
}

impl State {
    pub fn id(&self) -> SurfaceId {
        self.id
    }

    pub fn new(brightness: f32) -> (Self, cosmic::app::Task<AppMsg>) {
        let id = SurfaceId::unique();

        let mut cmds = vec![cosmic::surface::surface_task(simple_layer_shell(
            || LiveSettings::default(),
            move || SctkLayerSurfaceSettings {
                id,
                keyboard_interactivity: KeyboardInteractivity::None,
                namespace: "io.github.cosmic_utils.external-monitor-brightness.osd".into(),
                layer: Layer::Overlay,
                size: None,
                anchor: Anchor::BOTTOM,
                output: IcedOutput::Active,
                exclusive_zone: 0,
                margin: IcedMargin {
                    top: 0,
                    right: 0,
                    bottom: 48,
                    left: 0,
                },
                input_zone: Some(Vec::new()),
                ..Default::default()
            },
            None::<fn() -> Element<'static, cosmic::Action<Msg>>>,
        ))];

        let (cmd, timer_abort) = close_timer();
        cmds.push(cmd.map(move |x| cosmic::action::app(AppMsg::Osd(x))));

        (
            Self {
                id,
                brightness,
                timer_abort,
            },
            cosmic::Task::batch(cmds),
        )
    }

    // Re-use the OSD surface for a new brightness value; resets close timer.
    pub fn replace_brightness(&mut self, brightness: f32) -> Task<Msg> {
        self.brightness = brightness;
        self.timer_abort.abort();
        let (cmd, timer_abort) = close_timer();
        self.timer_abort = timer_abort;
        cmd
    }

    pub fn view(&self) -> Element<'_, Msg> {
        let icon = widget::icon::from_name("display-brightness-symbolic");
        let value = percent(self.brightness);
        let progress = value as f32 / 100.0;

        let osd_bar = widget::determinate_linear(progress)
            .girth(4)
            .width(Length::Fixed(266.0));

        let osd_contents = iced::widget::row![
            widget::container(icon.size(20)).center_x(Length::Fixed(32.0)),
            widget::text::body(format!("{}%", value))
                .width(Length::Fixed(32.0))
                .center(),
            widget::space::horizontal().width(Length::Fixed(8.0)),
            osd_bar,
        ]
        .align_y(Alignment::Center)
        .apply(widget::container)
        .width(Length::Fixed(392.0))
        .height(Length::Fixed(52.0))
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .class(cosmic::theme::Container::custom(move |theme| {
            widget::container::Style {
                text_color: Some(theme.cosmic().background(theme.transparent).on.into()),
                background: Some(
                    iced::Color::from(theme.cosmic().background(theme.transparent).base).into(),
                ),
                border: Border {
                    radius: theme.cosmic().radius_l().into(),
                    width: 1.0,
                    color: theme.cosmic().bg_divider().into(),
                },
                shadow: Default::default(),
                icon_color: Some(theme.cosmic().background(theme.transparent).on.into()),
                snap: true,
            }
        }));

        widget::autosize::autosize(
            widget::container(osd_contents)
                .align_x(Alignment::Center)
                .width(Length::Shrink)
                .align_bottom(Length::Shrink),
            OSD_ID.clone(),
        )
        .min_width(1.)
        .min_height(1.)
        .into()
    }

    pub fn update(self, msg: Msg) -> (Option<Self>, Task<Msg>) {
        match msg {
            Msg::Ignore => (Some(self), Task::none()),
            Msg::Close => (None, destroy_layer_surface(self.id)),
        }
    }
}
