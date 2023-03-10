use std::collections::HashMap;

use zbus::dbus_proxy;
use zbus::zvariant::Value;
use zbus::Connection;

#[dbus_proxy(
    interface = "org.sigxcpu.Feedback",
    default_path = "/org/sigxcpu/Feedback"
)]
trait Feedback {
    fn trigger_feedback(
        &self,
        app_name: &str,
        event: &str,
        hints: &HashMap<&str, &Value<'_>>,
        timeout: i32,
    ) -> zbus::Result<u32>;

    #[dbus_proxy(signal)]
    fn feedback_ended(&self, arg1: u32, arg2: u32) -> fdo::Result<()>;
}

#[derive(Clone)]
pub struct Feedbackd {
    connection: Connection,
}

impl Feedbackd {
    pub async fn new() -> Result<Self, zbus::Error> {
        let connection = Connection::session().await?;
        Ok(Self { connection })
    }

    pub async fn feedback(&self) -> Result<(), zbus::Error> {
        log::trace!("Providing feedback");
        let proxy = FeedbackProxy::new(&self.connection).await?;
        let _ = proxy
            .trigger_feedback(
                crate::config::APP_ID,
                "message-new-instant",
                &HashMap::new(),
                -1,
            )
            .await?;
        Ok(())
    }
}
