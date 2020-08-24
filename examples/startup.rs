//! Example application which notifies on startup.

use futures::FutureExt;
use tokio::sync::{broadcast, oneshot};
use tokio::{runtime, time};
use tokio_oga::commands::SessionStartup;
use tokio_oga::events::Event;

type ExError = Box<dyn std::error::Error + 'static>;

/// Delay before aborting with error.
const ABORT_DELAY_SECS: u8 = 30;

fn main() -> Result<(), ExError> {
    let mut rt = runtime::Runtime::new().expect("tokio runtime failure");
    rt.block_on(run())
}

/// Run core logic for startup notification.
async fn run() -> Result<(), ExError> {
    // Build and initialize the client.
    let builder = tokio_oga::OgaBuilder::default();
    let app = AppExample {
        delay_secs: ABORT_DELAY_SECS,
    };
    let mut client = builder.connect().await?;

    let term_chan = client.termination_chan();
    let events_chan = client.event_chan();
    let cmd_chan = client.command_chan();

    // Run until core logic is completed, or client experiences a failure,
    // or timeout is reached. Tasks run concurrently, the quickest one
    // completes, all the others are cancelled.
    tokio::select! {
        res = app.run_core_logic(events_chan, cmd_chan) => { res }
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

    /// Wait for `refresh` event, then send a `session-startup` command.
    async fn run_core_logic(
        &self,
        mut ch_incoming: broadcast::Receiver<Event>,
        mut ch_outgoing: tokio_oga::OgaCommandSender,
    ) -> Result<(), ExError> {
        use tokio::sync::broadcast::RecvError;

        // Loop until a `refresh` is observed.
        loop {
            let event = match ch_incoming.recv().await {
                Err(RecvError::Closed) => break async { Err("end of events stream") }.boxed(),
                Err(RecvError::Lagged(_)) => continue,
                Ok(ev) => ev,
            };

            match event {
                Event::Refresh(_v) => break async { Ok(()) }.boxed(),
                e => eprintln!("uninteresting event: {:?}", e),
            };
        }
        .await?;
        println!("Got 'refresh' event from host");

        // Send startup command.
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
