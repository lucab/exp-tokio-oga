use crate::commands;
use crate::{FramePlusChan, OgaError};
use futures::future::{AbortHandle, AbortRegistration, Abortable};
use tokio::sync::{mpsc, oneshot};
use tokio::time;

#[derive(Debug)]
pub(crate) struct PacemakerTask {
    abort: AbortRegistration,
    chan_to_manager: mpsc::Sender<FramePlusChan>,
    pause: u8,
}

impl PacemakerTask {
    /// Prepare a new pacemaker task, without starting it.
    pub(crate) fn new(
        chan_to_manager: mpsc::Sender<FramePlusChan>,
        pause: u8,
    ) -> (Self, AbortHandle) {
        let (handle, reg) = AbortHandle::new_pair();
        let task = Self {
            abort: reg,
            chan_to_manager,
            pause,
        };

        (task, handle)
    }

    /// Run this task.
    pub(crate) async fn run(self) -> OgaError {
        let exit = Self::process(self.chan_to_manager, self.pause);
        let res = Abortable::new(exit, self.abort).await;
        match res {
            Ok(Err(exit)) => exit,
            Ok(Ok(_)) => unreachable!(),
            Err(_) => OgaError::from("pacemaker task aborted"),
        }
    }

    /// Run the core processing logic for this task.
    pub(crate) async fn process(
        mut to_manager: mpsc::Sender<FramePlusChan>,
        pause: u8,
    ) -> Result<(), OgaError> {
        let pause = u64::from(pause);
        let beat = commands::Heartbeat::default();

        loop {
            let chan = oneshot::channel();
            to_manager
                .send((Box::new(beat.clone()), chan.0))
                .await
                .map_err(|e| OgaError::from(e.to_string()))?;
            let _ = chan.1.await;
            time::delay_for(time::Duration::from_secs(pause)).await;
        }
    }
}
