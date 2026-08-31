use anyhow::Context;
use iroh::address_lookup::dns::DnsAddressLookup;
use iroh::address_lookup::pkarr::PkarrPublisher;
use iroh::endpoint::presets;
use iroh::{Endpoint, RelayMode, SecretKey};
use iroh_blobs::ticket::BlobTicket;

use crate::cli::{AddrInfoOptions, CommonArgs};

fn apply_magic_bind(
    mut builder: iroh::endpoint::Builder,
    common: &CommonArgs,
) -> anyhow::Result<iroh::endpoint::Builder> {
    if let Some(addr) = common.magic_ipv4_addr {
        builder = builder.bind_addr(addr).context("bind IPv4 address")?;
    }
    if let Some(addr) = common.magic_ipv6_addr {
        builder = builder.bind_addr(addr).context("bind IPv6 address")?;
    }
    Ok(builder)
}

pub(crate) async fn bind_send_endpoint(
    secret: SecretKey,
    common: &CommonArgs,
    ticket_type: AddrInfoOptions,
) -> anyhow::Result<Endpoint> {
    let mut builder = Endpoint::builder(presets::N0)
        .alpns(vec![iroh_blobs::protocol::ALPN.to_vec()])
        .secret_key(secret)
        .relay_mode(RelayMode::from(common.relay.clone()));
    if ticket_type == AddrInfoOptions::Id {
        builder = builder.address_lookup(PkarrPublisher::n0_dns());
    }
    apply_magic_bind(builder, common)?
        .bind()
        .await
        .context("bind send endpoint")
}

pub(crate) async fn bind_recv_endpoint(
    secret: SecretKey,
    common: &CommonArgs,
    ticket: &BlobTicket,
) -> anyhow::Result<Endpoint> {
    let mut builder = Endpoint::builder(presets::N0)
        .alpns(vec![])
        .secret_key(secret)
        .relay_mode(RelayMode::from(common.relay.clone()));
    if ticket.addr().relay_urls().next().is_none() && ticket.addr().ip_addrs().next().is_none() {
        builder = builder.address_lookup(DnsAddressLookup::n0_dns());
    }
    apply_magic_bind(builder, common)?
        .bind()
        .await
        .context("bind receive endpoint")
}
