use crate::events::Event;
use crate::virtio::VirtioPort;
use crate::{FramePlusChan, OgaError};
use futures::future::{AbortHandle, AbortRegistration, Abortable};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, PollEvented, WriteHalf};
use tokio::sync::mpsc;

#[derive(Debug)]
pub(crate) struct ManagerTask {
    abort: AbortRegistration,
    dev: PollEvented<VirtioPort>,
    chan_incoming: mpsc::Receiver<FramePlusChan>,
    chan_outgoing: mpsc::Sender<Event>,
}

impl ManagerTask {
    pub(crate) fn new(
        dev: PollEvented<VirtioPort>,
        chan_incoming: mpsc::Receiver<FramePlusChan>,
        chan_outgoing: mpsc::Sender<Event>,
    ) -> (Self, AbortHandle) {
        let (handle, reg) = futures::future::AbortHandle::new_pair();
        let task = Self {
            abort: reg,
            dev,
            chan_incoming,
            chan_outgoing,
        };

        (task, handle)
    }

    /// Run this task.
    pub(crate) async fn run(self) -> OgaError {
        let exit = Self::process(self.dev, self.chan_incoming, self.chan_outgoing);
        let res = Abortable::new(exit, self.abort).await;
        log::trace!("manager done: {:?}", res);

        match res {
            Ok(Ok(_)) => unreachable!("manager cannot ever complete with success"),
            Ok(Err(exit)) => exit,
            Err(_) => OgaError::from("manager task aborted"),
        }
    }

    /// Run the core processing logic for this task.
    pub(crate) async fn process(
        dev: PollEvented<VirtioPort>,
        mut incoming_cmd: mpsc::Receiver<FramePlusChan>,
        mut outgoing_event: mpsc::Sender<Event>,
    ) -> Result<(), OgaError> {
        // Split the virtio port; the read half gets buffered and polled
        // for incoming events.
        let (mut dev_rd, mut dev_wr) = {
            let (rd, wr) = tokio::io::split(dev);
            let line_rd = BufReader::new(rd).lines();
            (line_rd, wr)
        };

        // Endless core loop; manager never completes with success.
        loop {
            tokio::select! {
                msg = dev_rd.next_line() => {
                    log::trace!("manager got event from virtio port");
                    let line = msg
                        .map_err(|e| OgaError::from(e.to_string()))?
                        .ok_or_else(|| OgaError::from("manager: end of unix socket stream"))?;

                    Self::forward_event(&mut outgoing_event, line).await?;
                },

                msg = incoming_cmd.recv() => {
                    log::trace!("manager got command from consumer");
                    let input = msg
                        .ok_or_else(|| OgaError::from("manager: end of incoming stream"))?;

                    Self::forward_command(&mut dev_wr, input).await?;
                }
            }
        }
    }

    /// Forward a command (consumer -> host).
    async fn forward_command(
        dev_wr: &mut WriteHalf<PollEvented<VirtioPort>>,
        input: FramePlusChan,
    ) -> Result<(), OgaError> {
        let (cmd, chan) = input;
        let data = cmd.as_frame()?;
        dev_wr
            .write_all(&data)
            .await
            .map_err(|e| OgaError::from(e.to_string()))?;
        dev_wr.flush().await.unwrap();
        let _ = chan.send(Ok(()));

        log::trace!("forwarded command: {:?}", cmd);
        Ok(())
    }

    /// Forward an event (host -> consumers).
    async fn forward_event(
        outgoing_ch: &mut mpsc::Sender<Event>,
        line: String,
    ) -> Result<(), OgaError> {
        let event = match Event::parse_frame(line.as_bytes()) {
            Ok(val) => val,
            Err(_) => {
                log::warn!("transient error, received unrecognized event: '{}'", &line);
                return Ok(());
            }
        };

        outgoing_ch
            .send(event.clone())
            .await
            .map_err(|e| OgaError::from(e.to_string()))?;

        log::trace!("forwarded event: {}", event);
        Ok(())
    }
}
