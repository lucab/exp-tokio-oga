# tokio-oga

An asynchronous client library for the oVirt Guest Agent (OGA) protocol.

This provides a client library, based on [Tokio](https://www.tokio.rs),
allowing a guest-agent application to asynchronously interact with an oVirt host
service like [VDSM](https://www.ovirt.org/develop/developer-guide/vdsm/vdsm.html).

It supports receiving [events](./events/index.html) from the host and sending
[commands](./commands/index.html) to it.

An end-to-end usage example is available under [`examples/`](examples).

References:

 * <https://resources.ovirt.org/old-site-files/wiki/Ovirt-guest-agent.pdf>
 * <https://github.com/oVirt/vdsm/blob/v4.40.25/lib/vdsm/virt/guestagent.py>
 * <https://github.com/oVirt/ovirt-guest-agent/blob/1.0.16/ovirt-guest-agent/OVirtAgentLogic.py>

## License

Licensed under either of

 * MIT license - <http://opensource.org/licenses/MIT>
 * Apache License, Version 2.0 - <http://www.apache.org/licenses/LICENSE-2.0>

at your option.
