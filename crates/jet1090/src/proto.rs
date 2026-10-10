//! Buf-generated protobuf messages and ConnectRPC service bindings.
#![allow(dead_code, unused_imports, clippy::all)]

/// Buf-generated descriptors served by gRPC reflection.
pub const FILE_DESCRIPTOR_SET: &[u8] =
    include_bytes!("generated/snapshot.fds.bin");

pub mod jet1090 {
    pub mod v1 {
        include!("generated/messages/jet1090.v1.rs");
        include!("generated/services/jet1090.v1.mod.rs");
    }
}
