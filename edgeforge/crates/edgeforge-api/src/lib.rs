// Copyright 2025 β ORI Inc. Canada. All Rights Reserved.
// Author: Awase Khirni Syed
pub mod dto;
pub mod middleware;
pub mod routes;
pub mod server;
pub mod state;
pub use server::{build_router, serve};
pub use state::AppState;
