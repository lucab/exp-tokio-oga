//! Example application which notifies on startup.

use tokio::sync::oneshot;
use tokio::{runtime, time};
use tokio_oga::commands::SessionStartup;

type ExError = Box<dyn std::error::Error + 'static>;

/// Delay before aborting with error.
const ABORT_DELAY_SECS: u8 = 30;

fn main() -> Result<(), ExError> {
    env_logger::Builder::from_default_env()
        .filter(Some("tokio_oga"), log::LevelFilter::Trace)
        .init();

    let mut rt = runtime::Runtime::new().expect("tokio runtime failure");
    rt.block_on(run())
}

/// Run core logic for startup notification.
async fn run() -> Result<(), ExError> {
    // Build and initialize the client.
    let builder = tokio_oga::OgaBuilder::default()
        .initial_heartbeat(Some(true))
        .heartbeat_interval(Some(0));
    let mut client = builder.connect().await?;

    let term_chan = client.termination_chan();
    let cmd_chan = client.command_chan();

    // Run until core logic is completed, or client experiences a failure,
    // or timeout is reached. Tasks run concurrently, the quickest one
    // completes, all the others are cancelled.
    let app = AppExample::default();
    tokio::select! {
        res = app.send_startup(cmd_chan) => { res }
        client_err = app.watch_termination(term_chan) => { Err(client_err) }
        _ = app.abort_delayed() => {
            Err("failed to notify startup: timed out".into())
        }
    }
}

/// Application logic.
struct AppExample {
    /// Running time before shutdown, in seconds.
    delay_secs: u8,
}

impl AppExample {
    /// Process client termination errors.
    async fn watch_termination(&self, chan: oneshot::Receiver<tokio_oga::OgaError>) -> ExError {
        let err = chan
            .await
            .unwrap_or_else(|_| "termination event, sender aborted".into());
        Box::new(err)
    }

    /// Send a `session-startup` command.
    async fn send_startup(
        &self,
        mut ch_outgoing: tokio_oga::OgaCommandSender,
    ) -> Result<(), ExError> {
        let startup_msg = SessionStartup::default();
        ch_outgoing.send(Box::new(startup_msg)).await?;
        println!("Sent 'session-startup' command to host");
        Ok(())
    }

    /// Abort after configured delay.
    async fn abort_delayed(&self) -> () {
        time::delay_for(time::Duration::from_secs(self.delay_secs.into())).await
    }
}

impl Default for AppExample {
    fn default() -> Self {
        AppExample {
            delay_secs: ABORT_DELAY_SECS,
        }
    }
}
