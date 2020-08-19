use tokio::sync::{broadcast, mpsc, oneshot};
use tokio::{runtime, time};
use tokio_oga::commands::AsFrame;
use tokio_oga::events::Event;

type ExError = Box<dyn std::error::Error + 'static>;

/// Application running time before graceful shutdown.
const SHUTDOWN_DELAY_SECS: u8 = 255;

fn main() -> Result<(), ExError> {
    let mut rt = runtime::Runtime::new().expect("tokio runtime failure");
    rt.block_on(run())
}

async fn run() -> Result<(), ExError> {
    let builder = tokio_oga::OgaBuilder::default();
    let app = AppExample {
        delay_secs: SHUTDOWN_DELAY_SECS,
    };

    let mut client = builder.handshake().await?;
    let term_chan = client.termination_chan();
    let events_chan = client.event_chan();
    let cmd_chan = client.command_chan();

    tokio::select! {
        res = app.run_core_logic(events_chan, cmd_chan) => { res }
        client_err = app.watch_termination(term_chan) => { Err(client_err) }
        _ = app.shutdown_delayed() => {
            eprintln!("Done, shutting down now.");
            Ok(())
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

    /// Process oVirt events.
    async fn run_core_logic(
        &self,
        mut ch_incoming: broadcast::Receiver<Event>,
        mut _ch_outgoing: mpsc::Sender<Box<dyn AsFrame>>,
    ) -> Result<(), ExError> {
        use tokio::sync::broadcast::RecvError;

        loop {
            let event = match ch_incoming.recv().await {
                Err(RecvError::Closed) => break async { Err("end of events stream".into()) },
                Err(RecvError::Lagged(_)) => continue,
                Ok(ev) => ev,
            };
            println!("got event from host: {:?}", event);

            match event {
                Event::Refresh(_v) => {}
                e => eprintln!("uninteresting event: {:?}", e),
            };
        }
        .await
    }

    /// Gracefully shutdown after configured delay.
    async fn shutdown_delayed(&self) -> () {
        time::delay_for(time::Duration::from_secs(self.delay_secs.into())).await
    }
}
