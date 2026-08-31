use std::collections::BTreeSet;

use iroh::{EndpointAddr, TransportAddr};
use iroh_blobs::Hash;

use crate::cli::{AddrInfoOptions, Format};

pub(crate) fn apply_options(addr: &mut EndpointAddr, opts: AddrInfoOptions) {
    match opts {
        AddrInfoOptions::Id => {
            addr.addrs = BTreeSet::default();
        }
        AddrInfoOptions::RelayAndAddresses => {}
        AddrInfoOptions::Relay => {
            addr.addrs = addr
                .addrs
                .iter()
                .filter(|addr| matches!(addr, TransportAddr::Relay(_)))
                .cloned()
                .collect();
        }
        AddrInfoOptions::Addresses => {
            addr.addrs = addr
                .addrs
                .iter()
                .filter(|addr| matches!(addr, TransportAddr::Ip(_)))
                .cloned()
                .collect();
        }
    }
}

pub(crate) fn print_hash(hash: &Hash, format: Format) -> String {
    match format {
        Format::Hex => hash.to_hex(),
        Format::Cid => hash.to_string(),
    }
}
