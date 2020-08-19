use crate::commands::AsFrame;
use crate::events::Event;
use crate::OgaError;
use futures::future::{AbortHandle, AbortRegistration, Abortable};
use tokio::sync::{broadcast, mpsc};

#[derive(Debug)]
pub(crate) struct DispatcherTask {
    abort: AbortRegistration,
    chan_from_app: mpsc::Receiver<Box<dyn AsFrame>>,
    chan_from_manager: mpsc::Receiver<Event>,
    chan_to_app: broadcast::Sender<Event>,
    chan_to_manager: mpsc::Sender<Box<dyn AsFrame>>,
}

impl DispatcherTask {
    pub(crate) fn new(
        chan_from_app: mpsc::Receiver<Box<dyn AsFrame>>,
        chan_from_manager: mpsc::Receiver<Event>,
        chan_to_app: broadcast::Sender<Event>,
        chan_to_manager: mpsc::Sender<Box<dyn AsFrame>>,
    ) -> (Self, AbortHandle) {
        let (handle, reg) = AbortHandle::new_pair();
        let task = Self {
            abort: reg,
            chan_from_app,
            chan_from_manager,
            chan_to_app,
            chan_to_manager,
        };

        (task, handle)
    }

    /// Run this task.
    pub(crate) async fn run(self) -> OgaError {
        let exit = Self::process(
            self.chan_from_app,
            self.chan_from_manager,
            self.chan_to_app,
            self.chan_to_manager,
        );
        let res = Abortable::new(exit, self.abort).await;
        match res {
            Ok(Err(exit)) => exit,
            Ok(Ok(_)) => unreachable!(),
            Err(_) => OgaError::from("dispatcher task aborted"),
        }
    }

    /// Run the core processing logic for this task.
    pub(crate) async fn process(
        mut from_app: mpsc::Receiver<Box<dyn AsFrame>>,
        mut from_manager: mpsc::Receiver<Event>,
        to_app: broadcast::Sender<Event>,
        mut to_manager: mpsc::Sender<Box<dyn AsFrame>>,
    ) -> Result<(), OgaError> {
        loop {
            tokio::select! {
                msg = from_manager.recv() => {
                    let event = msg.ok_or_else(|| OgaError::from("from_manager sender dropped"))?;
                    let _ = to_app.send(event);
                },
                msg = from_app.recv() => {
                    let cmd = msg.ok_or_else(|| OgaError::from("from_app sender dropped"))?;
                    to_manager.send(cmd)
                        .await
                        .map_err(|e| OgaError::from(e.to_string()))?;
                },
            }
        }
    }
}
