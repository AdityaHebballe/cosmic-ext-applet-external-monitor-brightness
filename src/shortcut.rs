use cosmic::iced::{
    futures::{SinkExt, Stream},
    stream,
};

use crate::app::{APPID, AppMsg};

const OBJECT_PATH: &str = "/io/github/cosmic_utils/ExternalMonitorBrightness";
const INTERFACE: &str = "io.github.cosmic_utils.ExternalMonitorBrightness";

#[derive(Clone)]
struct ShortcutService {
    output: cosmic::iced::futures::channel::mpsc::Sender<AppMsg>,
}

#[zbus::interface(name = "io.github.cosmic_utils.ExternalMonitorBrightness")]
impl ShortcutService {
    async fn increase(&self) {
        self.change(AppMsg::IncreaseGlobalBrightness).await;
    }

    async fn decrease(&self) {
        self.change(AppMsg::DecreaseGlobalBrightness).await;
    }
}

impl ShortcutService {
    async fn change(&self, message: AppMsg) {
        let mut output = self.output.clone();
        if output.send(message).await.is_err() {
            error!("brightness shortcut receiver closed");
        }
    }
}

/// Serve shortcut requests for as long as the applet is running.
pub fn sub() -> impl Stream<Item = AppMsg> {
    stream::channel(
        1,
        |output: cosmic::iced::futures::channel::mpsc::Sender<AppMsg>| async move {
            let service = ShortcutService { output };
            let builder = match zbus::connection::Builder::session() {
                Ok(builder) => builder,
                Err(err) => {
                    error!("can't connect to the session bus for shortcuts: {err}");
                    return;
                }
            };
            let builder = match builder.name(APPID) {
                Ok(builder) => builder,
                Err(err) => {
                    error!("can't reserve shortcut bus name: {err}");
                    return;
                }
            };
            let builder = match builder.serve_at(OBJECT_PATH, service) {
                Ok(builder) => builder,
                Err(err) => {
                    error!("can't expose brightness shortcut service: {err}");
                    return;
                }
            };

            let _connection = match builder.build().await {
                Ok(connection) => connection,
                Err(err) => {
                    error!("can't start brightness shortcut service: {err}");
                    return;
                }
            };

            std::future::pending::<()>().await;
        },
    )
}

/// Ask the running applet to change brightness without opening DDC devices.
pub fn request_change(delta: f32) -> zbus::Result<()> {
    let connection = zbus::blocking::Connection::session()?;
    let proxy = zbus::blocking::Proxy::new(&connection, APPID, OBJECT_PATH, INTERFACE)?;
    let method = if delta.is_sign_positive() {
        "Increase"
    } else {
        "Decrease"
    };
    proxy.call::<_, _, ()>(method, &())?;
    Ok(())
}
