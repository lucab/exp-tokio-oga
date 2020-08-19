use crate::commands::AsFrame;
use crate::events::Event;
use crate::OgaError;
use futures::future::{AbortHandle, AbortRegistration, Abortable};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tokio::sync::mpsc;

#[derive(Debug)]
pub(crate) struct ManagerTask {
    abort: AbortRegistration,
    sock: UnixStream,
    chan_incoming: mpsc::Receiver<Box<dyn AsFrame>>,
    chan_outgoing: mpsc::Sender<Event>,
}

impl ManagerTask {
    pub(crate) fn new(
        sock: UnixStream,
        chan_incoming: mpsc::Receiver<Box<dyn AsFrame>>,
        chan_outgoing: mpsc::Sender<Event>,
    ) -> (Self, AbortHandle) {
        let (handle, reg) = futures::future::AbortHandle::new_pair();
        let task = Self {
            abort: reg,
            sock,
            chan_incoming,
            chan_outgoing,
        };

        (task, handle)
    }

    /// Run this task.
    pub(crate) async fn run(self) -> OgaError {
        let exit = Self::process(self.sock, self.chan_incoming, self.chan_outgoing);
        let res = Abortable::new(exit, self.abort).await;
        match res {
            Ok(Err(exit)) => exit,
            Ok(Ok(_)) => unreachable!(),
            Err(_) => OgaError::from("manager task aborted"),
        }
    }

    /// Run the core processing logic for this task.
    pub(crate) async fn process(
        mut sock: UnixStream,
        mut incoming_cmd: mpsc::Receiver<Box<dyn AsFrame>>,
        mut outgoing_event: mpsc::Sender<Event>,
    ) -> Result<(), OgaError> {
        let (mut sock_rd, mut sock_wr) = {
            let (rd, wr) = sock.split();
            let line_rd = BufReader::new(rd).lines();
            (line_rd, wr)
        };

        loop {
            tokio::select! {
                msg = sock_rd.next_line() => {
                    log::trace!("manager: socket line");
                    let line = msg
                        .map_err(|e| OgaError::from(e.to_string()))?
                        .ok_or_else(|| OgaError::from("manager: end of unix socket stream"))?;
                    let event: Event = match serde_json::from_str(&line) {
                        Ok(val) => { val },
                        Err(_) => {
                            // XXX(lucab): log::warn here.
                            eprintln!("failed to decode event: '{}'", &line);
                            continue
                        },
                    };
                    outgoing_event.send(event).await
                        .map_err(|e| OgaError::from(e.to_string()))?;
                },
                msg = incoming_cmd.recv() => {
                    log::trace!("manager: incoming cmd");
                    let cmd = msg
                        .ok_or_else(|| OgaError::from("manager: end of incoming stream"))?;
                    let data = cmd.as_frame()?;
                    sock_wr.write(&data).await
                        .map_err(|e| OgaError::from(e.to_string()))?;
                }
            }
        }
    }
}
