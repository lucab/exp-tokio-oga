/*!
An asynchronous client library for the oVirt Guest Agent (OGA) protocol.

This provides a client library, based on [Tokio](https://www.tokio.rs),
allowing a guest-agent application to asynchronously interact with an oVirt host
service like [VDSM](https://www.ovirt.org/develop/developer-guide/vdsm/vdsm.html).

It supports receiving [events](./events/index.html) from the host and sending
[commands](./commands/index.html) to it.

The entrypoint for client initialization is [OgaClient::builder()](struct.OgaClient.html#method.builder).

References:
 * <https://resources.ovirt.org/old-site-files/wiki/Ovirt-guest-agent.pdf>
 * <https://github.com/oVirt/vdsm/blob/v4.40.25/lib/vdsm/virt/guestagent.py>
 * <https://github.com/oVirt/ovirt-guest-agent/blob/1.0.16/ovirt-guest-agent/OVirtAgentLogic.py>
!*/

/*
This internally starts the following tasks:
 * Pacemaker  - heartbeat generator.
 * Manager    - socket manager towards the hypervisor service.
 * Dispatcher - channel handler towards library consumers.
 * Runner     - top-level umbrella and client engine.
*/

#![deny(missing_debug_implementations)]

pub mod commands;
mod errors;
pub mod events;
mod tasks;
mod virtio;

use crate::commands::AsFrame;
pub use crate::errors::OgaError;
use crate::virtio::VirtioPort;
use futures::future::{AbortHandle, TryFutureExt};
use std::path::{Path, PathBuf};
use tokio::io::{AsyncWriteExt, PollEvented};
use tokio::sync::{broadcast, mpsc, oneshot};
use tokio::time::{self, Duration};

/// Tuple with pending frame and channel for the result.
type FramePlusChan = (Box<dyn AsFrame>, oneshot::Sender<Result<(), OgaError>>);

/// Default path to the VirtIO device.
pub static DEFAULT_VIRTIO_PATH: &str = "/dev/virtio-ports/ovirt-guest-agent.0";

/// Configuration and builder for `OgaClient`.
#[derive(Clone, Debug)]
pub struct OgaBuilder {
    commands_buffer: usize,
    connect_timeout: u8,
    events_buffer: usize,
    heartbeat_secs: u8,
    initial_heartbeat: bool,
    virtio: PathBuf,
}

impl Default for OgaBuilder {
    fn default() -> Self {
        Self {
            commands_buffer: 10,
            connect_timeout: 5,
            events_buffer: 10,
            heartbeat_secs: 5,
            initial_heartbeat: true,
            virtio: PathBuf::from(DEFAULT_VIRTIO_PATH),
        }
    }
}

impl OgaBuilder {
    /// Return a builder with default configuration settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether to send an heartbeat on connect (default: true).
    pub fn initial_heartbeat(mut self, arg: Option<bool>) -> Self {
        let setting = arg.unwrap_or(true);
        self.initial_heartbeat = setting;
        self
    }

    /// Seconds between heartbeats, or 0 to disable (default: 5).
    pub fn heartbeat_interval(mut self, arg: Option<u8>) -> Self {
        let setting = arg.unwrap_or(5);
        self.heartbeat_secs = setting;
        self
    }

    /// Path to the VirtIO serial port (default: `DEFAULT_VIRTIO_PATH`).
    pub fn device_path(mut self, arg: Option<impl AsRef<Path>>) -> Self {
        let setting = match arg {
            Some(p) => p.as_ref().to_path_buf(),
            None => PathBuf::from(DEFAULT_VIRTIO_PATH),
        };
        self.virtio = setting;
        self
    }

    /// Connect, initialize, and return a client.
    pub async fn connect(self) -> Result<OgaClient, OgaError> {
        let mut dev = VirtioPort::open(&self.virtio)?.evented()?;
        log::debug!("virtio port found at '{}'", &self.virtio.display());

        if self.initial_heartbeat {
            let conn_timeout = Duration::from_secs(u64::from(self.connect_timeout));
            time::timeout(conn_timeout, Self::send_heartbeat(&mut dev))
                .await
                .map_err(|e| format!("failed to send initial heartbeat: {}", e))??;
            log::trace!("initial heartbeat sent");
        }

        let client = OgaClient::initialize(self, dev).await;
        Ok(client)
    }

    async fn send_heartbeat(dev: &mut PollEvented<VirtioPort>) -> Result<(), errors::OgaError> {
        let frame = commands::Heartbeat::default().as_frame()?;
        dev.write_all(&frame).await.map_err(|e| e.to_string())?;
        dev.flush().await.map_err(|e| e.to_string().into())
    }
}

/// Client for oVirt Guest Agent protocol.
#[derive(Debug)]
pub struct OgaClient {
    termination: Option<oneshot::Receiver<OgaError>>,
    abortable_tasks: Vec<AbortHandle>,
    from_app: mpsc::Sender<FramePlusChan>,
    to_app: broadcast::Sender<crate::events::Event>,
}

impl OgaClient {
    /// Return a client builder with default configuration settings.
    pub fn builder() -> OgaBuilder {
        OgaBuilder::default()
    }

    /// Initialize and run a client.
    ///
    /// This internally starts the following tasks:
    ///  * Pacemaker  - heartbeat generator.
    ///  * Manager    - socket manager towards the hypervisor service.
    ///  * Dispatcher - channel handler towards library consumers.
    ///  * Runner     - top-level umbrella and client engine.
    async fn initialize(builder: OgaBuilder, dev: PollEvented<VirtioPort>) -> Self {
        let (runner_abort, runner_reg) = futures::future::AbortHandle::new_pair();

        // Channels.
        let termination_chan = oneshot::channel();
        let from_app_chan = mpsc::channel(builder.commands_buffer);
        let to_manager_chan = mpsc::channel(builder.commands_buffer);
        let from_manager_chan = mpsc::channel(builder.events_buffer);
        let to_app_chan = {
            let bcast = broadcast::channel(builder.events_buffer);
            drop(bcast.1);
            bcast.0
        };

        let (dispatcher, dispatcher_abort) = tasks::DispatcherTask::new(
            from_app_chan.1,
            from_manager_chan.1,
            to_app_chan.clone(),
            to_manager_chan.0.clone(),
        );
        let (manager, manager_abort) =
            tasks::ManagerTask::new(dev, to_manager_chan.1, from_manager_chan.0);
        let (pacemaker, pacemaker_abort) =
            tasks::PacemakerTask::new(to_manager_chan.0, builder.heartbeat_secs);

        let abortable_tasks = vec![
            pacemaker_abort,
            dispatcher_abort,
            manager_abort,
            runner_abort,
        ];
        let client = Self {
            termination: Some(termination_chan.1),
            abortable_tasks,
            from_app: from_app_chan.0,
            to_app: to_app_chan,
        };

        tokio::spawn({
            let inner = Self::run_tasks(termination_chan.0, manager, pacemaker, dispatcher);
            futures::future::Abortable::new(inner, runner_reg)
        });
        client
    }

    /// Run all internal tasks.
    async fn run_tasks(
        err_chan: oneshot::Sender<OgaError>,
        manager: tasks::ManagerTask,
        pacemaker: tasks::PacemakerTask,
        dispatcher: tasks::DispatcherTask,
    ) {
        // Manager.
        let manager_task = tokio::spawn(manager.run())
            .map_ok_or_else(|_| OgaError::from("manager task failed"), |e| e);

        // Pacemaker.
        let pacemaker_task = tokio::spawn(pacemaker.run())
            .map_ok_or_else(|_| OgaError::from("pacemaker task failed"), |e| e);

        // Dispatcher.
        let dispatcher_task = tokio::spawn(dispatcher.run())
            .map_ok_or_else(|_| OgaError::from("service task failed"), |e| e);

        let err = tokio::select! {
            ret = dispatcher_task => { ret },
            ret = manager_task => { ret },
            ret = pacemaker_task => { ret },
        };

        // Forward termination failure to the application.
        if let Err(fail) = err_chan.send(err) {
            log::error!("termination failure: {}", fail);
        }
    }

    /// Return a channel (write-half) for sending guest commands.
    pub fn command_chan(&mut self) -> OgaCommandSender {
        let from_app = self.from_app.clone();
        OgaCommandSender { from_app }
    }

    /// Return a channel (read-half) for receiving events from the host.
    pub fn event_chan(&mut self) -> broadcast::Receiver<crate::events::Event> {
        self.to_app.subscribe()
    }

    /// Return a channel (read-half) for receiving termination event notifications.
    pub fn termination_chan(&mut self) -> oneshot::Receiver<OgaError> {
        self.termination.take().unwrap_or_else(|| {
            let (send_ch, recv_ch) = oneshot::channel();
            let _ = send_ch.send(OgaError::from("termination channel unavailable"));
            recv_ch
        })
    }
}

impl Drop for OgaClient {
    fn drop(&mut self) {
        for task in &self.abortable_tasks {
            task.abort()
        }
    }
}

#[derive(Clone, Debug)]
/// Channel for sending commands to the host.
pub struct OgaCommandSender {
    from_app: mpsc::Sender<FramePlusChan>,
}

impl OgaCommandSender {
    /// Send a command to the host.
    pub async fn send(&mut self, cmd: Box<dyn commands::AsFrame>) -> Result<(), OgaError> {
        let err_chan = oneshot::channel();
        self.from_app
            .send((cmd, err_chan.0))
            .await
            .map_err(|e| OgaError::from(e.to_string()))?;
        err_chan
            .1
            .await
            .map_err(|e| OgaError::from(e.to_string()))?
    }
}
