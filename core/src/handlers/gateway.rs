use axum::{extract::Path, routing::put, Json, Router};

use crate::{error::Error, port_manager::PortManager, AppState};

use super::util::AuthedUser;

pub async fn open_port(requester: AuthedUser, Path(port): Path<u16>) -> Result<Json<()>, Error> {
    requester.require_owner()?;
    // The UPnP gateway search can take seconds; it does not need the port manager, so its
    // lock is not held while it runs.
    PortManager::open_upnp_port(port).await?;
    Ok(Json(()))
}

pub fn get_gateway_routes(state: AppState) -> Router {
    Router::new()
        .route("/gateway/open_port/{port}", put(open_port))
        .with_state(state)
}
