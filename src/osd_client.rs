use tokio::sync::OnceCell;

const BUS_NAME: &str = "io.github.cosmic_utils.ExternalOsd";
const OBJECT_PATH: &str = "/io/github/cosmic_utils/ExternalOsd";
const INTERFACE: &str = "io.github.cosmic_utils.ExternalOsd";

static CONNECTION: OnceCell<zbus::Connection> = OnceCell::const_new();

pub async fn show_brightness(brightness: f32) -> zbus::Result<()> {
    let connection = CONNECTION
        .get_or_try_init(zbus::Connection::session)
        .await?;
    let proxy = zbus::Proxy::new(&connection, BUS_NAME, OBJECT_PATH, INTERFACE).await?;
    proxy
        .call::<_, _, ()>("ShowBrightness", &(f64::from(brightness)))
        .await
}
