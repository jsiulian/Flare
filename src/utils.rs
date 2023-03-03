use gio::prelude::FileExt;
use gtk::gio;

#[macro_export]
macro_rules! gspawn {
    ($future:expr) => {
        let ctx = glib::MainContext::default();
        ctx.spawn_local($future);
    };
}

#[macro_export]
macro_rules! tspawn {
    ($future:expr) => {
        $crate::TOKIO_RUNTIME.spawn($future)
    };
}

pub fn is_flatpak() -> bool {
    let file = gio::File::for_path("/.flatpak-info");
    file.query_exists(gio::Cancellable::NONE)
}

pub async fn is_online() -> bool {
    log::trace!("Checking online status");
    tokio::net::TcpStream::connect("detectportal.firefox.com:80")
        .await
        .is_ok()
}

pub async fn await_online() {
    while !is_online().await {
        log::trace!("Currently offline. Waiting two seconds");
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    }
}

pub async fn await_suspend_wakeup() -> ashpd::Result<()> {
    let login1 = crate::login1::Login1::new().await?;

    log::trace!("Awaiting sleep change.");
    while login1.receive_sleep().await? {
        log::trace!("Going to sleep. Do nothing.");
    }

    log::trace!("Waking up from suspend.");

    Ok(())
}

pub async fn await_suspend_wakeup_online() -> ashpd::Result<()> {
    await_suspend_wakeup().await?;
    await_online().await;
    Ok(())
}
