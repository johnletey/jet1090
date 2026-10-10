use std::sync::Arc;

use connectrpc::{
    ConnectError, Encodable, RequestContext, Response, ServiceRequest,
    ServiceResult, ServiceStream,
};
use futures::{stream, StreamExt};
use tokio::sync::broadcast::error::RecvError;

use crate::proto::jet1090::v1::{
    AircraftService, AircraftSnapshot, GetSnapshotRequest, GetSnapshotResponse,
    SensorMetadata, SubscribeSnapshotsRequest, SubscribeSnapshotsResponse,
};
use crate::snapshot::Snapshot;
use crate::SharedState;

pub struct AircraftApi(pub Arc<SharedState>);

impl AircraftApi {
    async fn current(&self) -> GetSnapshotResponse {
        let states = self.0.state_vectors.read().await;
        GetSnapshotResponse {
            aircraft: states
                .values()
                .map(|state| (&state.cur).into())
                .collect(),
            ..Default::default()
        }
    }

    async fn updates(&self) -> ServiceStream<SubscribeSnapshotsResponse> {
        // Writers publish under this lock, so each change is either in the
        // initial snapshot or queued after it.
        let states = self.0.state_vectors.read().await;
        let rx = self.0.snapshot_tx.subscribe();
        let initial = SubscribeSnapshotsResponse {
            reset: true,
            aircraft: states
                .values()
                .map(|state| (&state.cur).into())
                .collect(),
            ..Default::default()
        };
        drop(states);
        let mut quit = self.0.quit_tx.subscribe();

        let updates = stream::unfold(Some(rx), |rx| async move {
            let mut rx = rx?;
            match rx.recv().await {
                Ok(update) => Some((Ok((*update).clone()), Some(rx))),
                Err(RecvError::Lagged(_)) => Some((
                    Err(ConnectError::resource_exhausted(
                        "snapshot subscriber fell behind. Reconnect to resync",
                    )),
                    None,
                )),
                Err(RecvError::Closed) => None,
            }
        });
        Box::pin(
            stream::once(async { Ok(initial) })
                .chain(updates)
                .take_until(async move {
                    let _ = quit.wait_for(|quit| *quit).await;
                }),
        )
    }
}

impl AircraftService for AircraftApi {
    async fn get_snapshot<'a>(
        &'a self,
        _ctx: RequestContext,
        _request: ServiceRequest<'_, GetSnapshotRequest>,
    ) -> ServiceResult<impl Encodable<GetSnapshotResponse> + Send + use<'a>>
    {
        Response::ok(self.current().await)
    }

    async fn subscribe_snapshots(
        &self,
        _ctx: RequestContext,
        _request: ServiceRequest<'_, SubscribeSnapshotsRequest>,
    ) -> ServiceResult<
        ServiceStream<
            impl Encodable<SubscribeSnapshotsResponse> + Send + use<>,
        >,
    > {
        Response::stream_ok(self.updates().await)
    }
}

impl From<&Snapshot> for AircraftSnapshot {
    fn from(snapshot: &Snapshot) -> Self {
        Self {
            icao24: snapshot.icao24.clone(),
            firstseen: snapshot.firstseen,
            lastseen: snapshot.lastseen,
            callsign: snapshot.callsign.clone(),
            registration: snapshot.registration.clone(),
            typecode: snapshot.typecode.clone(),
            squawk: snapshot.squawk.map(|code| code.to_string()),
            latitude: snapshot.latitude,
            longitude: snapshot.longitude,
            altitude: snapshot.altitude,
            selected_altitude: snapshot.selected_altitude.map(u32::from),
            groundspeed: snapshot.groundspeed,
            vertical_rate: snapshot.vertical_rate.map(i32::from),
            track: snapshot.track,
            ias: snapshot.ias.map(u32::from),
            tas: snapshot.tas.map(u32::from),
            mach: snapshot.mach,
            roll: snapshot.roll,
            heading: snapshot.heading,
            nacp: snapshot.nacp.map(u32::from),
            count: snapshot.count as u64,
            metadata: snapshot
                .metadata
                .iter()
                .map(|metadata| SensorMetadata {
                    system_timestamp: metadata.system_timestamp,
                    gnss_timestamp: metadata.gnss_timestamp,
                    nanoseconds: metadata.nanoseconds,
                    rssi: metadata.rssi,
                    serial: metadata.serial,
                    name: metadata.name.clone(),
                    ..Default::default()
                })
                .collect(),
            airport: snapshot.airport.clone(),
            ..Default::default()
        }
    }
}
