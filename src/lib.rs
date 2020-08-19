/*!
An asynchronous client library for the oVirt Guest Agent (OGA) protocol.

This provides a client library, based on [Tokio](https://www.tokio.rs),
allowing a guest-agent application to asynchronously interact with an oVirt host
service like [VDSM](https://www.ovirt.org/develop/developer-guide/vdsm/vdsm.html).

It supports receiving [events](./events/index.html) from the host and sending
[commands](./commands/index.html) to it.

The entrypoint for client initialization is [OgaClient::builder()](struct.OgaClient.html#method.builder).

An end-to-end usage example is available under [`examples`](./examples).

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

use crate::commands::AsFrame;
pub use crate::errors::OgaError;
use futures::future::AbortHandle;
use futures::future::TryFutureExt;
use std::path::PathBuf;
use tokio::io::AsyncWriteExt;
use tokio::net::UnixStream;
use tokio::sync::{broadcast, mpsc, oneshot};

/// Default path to the Unix Domanin Socket (without udev symlinking).
pub static DEFAULT_BARE_VIRTIO_PATH: &str = "/dev/virtio-ports/ovirt-guest-agent.0";

/// Default path to the Unix Domain Socket (with udev symlinking).
pub static DEFAULT_UDEV_VIRTIO_PATH: &str = "/dev/virtio-ports/com.redhat.rhevm.vdsm";

/// Configuration and builder for `OgaClient`.
#[derive(Clone, Debug)]
pub struct OgaBuilder {
    virtio: PathBuf,
    heartbeat_secs: u8,
}

impl Default for OgaBuilder {
    fn default() -> Self {
        Self {
            virtio: PathBuf::from(DEFAULT_BARE_VIRTIO_PATH),
            heartbeat_secs: 5,
        }
    }
}

impl OgaBuilder {
    /// Return a builder with default configuration settings.
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn handshake(self) -> Result<OgaClient, OgaError> {
        let mut uds = UnixStream::connect(&self.virtio)
            .await
            .map_err(|_| format!("failed to connect to '{}'", self.virtio.display()))?;
        let frame = commands::Heartbeat::default().as_frame()?;
        uds.write_all(&frame)
            .await
            .map_err(|_| "failed to send heartbeat")?;

        let client = OgaClient::initialize(self, uds).await;
        Ok(client)
    }
}

/// Client for oVirt Guest Agent protocol.
#[derive(Debug)]
pub struct OgaClient {
    termination: Option<oneshot::Receiver<OgaError>>,
    abortable_tasks: Vec<AbortHandle>,
    from_app: mpsc::Sender<Box<dyn commands::AsFrame>>,
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
    async fn initialize(builder: OgaBuilder, conn: UnixStream) -> Self {
        let (runner_abort, runner_reg) = futures::future::AbortHandle::new_pair();

        // Channels.
        let termination_chan = oneshot::channel();
        let from_app_chan = mpsc::channel(128);
        let to_manager_chan = mpsc::channel(128);
        let from_manager_chan = mpsc::channel(128);
        let to_app_chan = {
            let bcast = broadcast::channel(128);
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
            tasks::ManagerTask::new(conn, to_manager_chan.1, from_manager_chan.0);
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
    pub fn command_chan(&mut self) -> mpsc::Sender<Box<dyn commands::AsFrame>> {
        self.from_app.clone()
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
