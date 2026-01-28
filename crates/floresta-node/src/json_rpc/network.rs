//! This module holds all RPC server side methods for interacting with our node's network stack.

use std::net::SocketAddr;

use bitcoin::Network;
use floresta_wire::node_interface::PeerInfo;
use serde_json::json;
use serde_json::Value;

use super::res::JsonRpcError;
use super::server::RpcChain;
use super::server::RpcImpl;

type Result<T> = std::result::Result<T, JsonRpcError>;

impl<Blockchain: RpcChain> RpcImpl<Blockchain> {
    pub(crate) async fn ping(&self) -> Result<bool> {
        self.node
            .ping()
            .await
            .map_err(|e| JsonRpcError::Node(e.to_string()))
    }

    pub(crate) async fn add_node(
        &self,
        node: String,
        command: String,
        v2transport: bool,
    ) -> Result<Value> {
        let node = node.split(':').collect::<Vec<&str>>();
        let (ip, port) = if node.len() == 2 {
            (
                node[0],
                node[1].parse().map_err(|_| JsonRpcError::InvalidPort)?,
            )
        } else {
            // TODO: use `NetworkExt` to append the correct port
            // once https://github.com/rust-bitcoin/rust-bitcoin/pull/4639 makes it into a release.
            match self.network {
                Network::Bitcoin => (node[0], 8333),
                Network::Signet => (node[0], 38333),
                Network::Testnet => (node[0], 18333),
                Network::Testnet4 => (node[0], 48333),
                Network::Regtest => (node[0], 18444),
            }
        };

        let peer = ip.parse().map_err(|_| JsonRpcError::InvalidAddress)?;

        let _ = match command.as_str() {
            "add" => self.node.add_peer(peer, port, v2transport).await,
            "remove" => self.node.remove_peer(peer, port).await,
            "onetry" => self.node.onetry_peer(peer, port, v2transport).await,
            _ => return Err(JsonRpcError::InvalidAddnodeCommand),
        };

        Ok(json!(null))
    }

    pub(crate) async fn disconnect_node(
        &self,
        node_address: String,
        node_id: Option<u32>,
    ) -> Result<Value> {
        let (peer_addr, peer_port) = match (node_address.is_empty(), node_id) {
            // Reference the peer by it's IP address and port.
            (false, None) => {
                // Try to parse `node_address` into a `SocketAddr`.
                // This will handle IPv4:port and IPv6:port.
                let socket_addr = node_address
                    .parse::<SocketAddr>()
                    .map_err(|_| JsonRpcError::InvalidAddress)?;

                (socket_addr.ip(), socket_addr.port())
            }
            // Reference the peer by it's ID.
            (true, Some(node_id)) => {
                let peer_info = self
                    .node
                    .get_peer_info()
                    .await
                    .map_err(|e| JsonRpcError::Node(e.to_string()))?;

                let peer = peer_info
                    .iter()
                    .find(|peer| peer.id == node_id)
                    .ok_or(JsonRpcError::PeerNotFound)?;

                (peer.address.ip(), peer.address.port())
            }
            // Both address and ID were provided, or neither was provided.
            _ => {
                return Err(JsonRpcError::InvalidDisconnectNodeCommand);
            }
        };

        let disconnected = self
            .node
            .disconnect_peer(peer_addr, peer_port)
            .await
            .map_err(|e| JsonRpcError::Node(e.to_string()))?;

        if !disconnected {
            return Err(JsonRpcError::PeerNotFound);
        }

        Ok(json!(null))
    }

    pub(crate) async fn get_peer_info(&self) -> Result<Vec<PeerInfo>> {
        self.node
            .get_peer_info()
            .await
            .map_err(|_| JsonRpcError::Node("Failed to get peer information".to_string()))
    }
}
