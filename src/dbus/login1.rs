use futures::StreamExt;
use zbus::{proxy, Connection};

#[proxy(
    interface = "org.freedesktop.login1.Manager",
    default_service = "org.freedesktop.login1",
    default_path = "/org/freedesktop/login1"
)]
trait Login1 {
    #[zbus(signal)]
    fn prepare_for_sleep(&self, arg1: bool) -> fdo::Result<()>;
}

pub struct Login1 {
    connection: Connection,
}

impl Login1 {
    pub async fn new() -> Result<Self, zbus::Error> {
        Ok(Self {
            connection: Connection::system().await?,
        })
    }

    pub async fn receive_sleep(&self) -> Result<bool, zbus::Error> {
        let proxy = Login1Proxy::new(&self.connection).await?;
        proxy
            .receive_prepare_for_sleep()
            .await?
            .next()
            .await
            .expect("Signal never received")
            .args()
            .map(|a| *a.arg1())
    }
}
