use super::*;
use base64::{engine::general_purpose::STANDARD, Engine};
use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};

pub fn advertise(host: &HostIdentity) -> AppResult<ServiceDaemon> {
    let daemon = ServiceDaemon::new()
        .map_err(|_| AppError::storage("No se pudo publicar la recepción en la red"))?;
    let encoded = STANDARD.encode(host.certificate.as_bytes());
    let mut properties = HashMap::from([
        ("station_id".to_string(), host.station_id.clone()),
        ("name".into(), host.name.clone()),
        ("fingerprint".into(), host.fingerprint.clone()),
    ]);
    for (i, chunk) in encoded.as_bytes().chunks(180).enumerate() {
        properties.insert(
            format!("cert{i}"),
            String::from_utf8_lossy(chunk).into_owned(),
        );
    }
    let service = ServiceInfo::new(
        SERVICE_TYPE,
        &host.station_id,
        &format!("nightdesk-{}.local.", host.station_id),
        host.address.as_str(),
        host.port,
        properties,
    )
    .map_err(|_| AppError::storage("No se pudo anunciar la dirección local"))?;
    daemon
        .register(service)
        .map_err(|_| AppError::storage("No se pudo anunciar recepción"))?;
    Ok(daemon)
}
pub fn discover() -> AppResult<Vec<HostIdentity>> {
    let daemon = ServiceDaemon::new().map_err(|_| {
        AppError::storage("No se pudo buscar en la red local. Usá la conexión manual")
    })?;
    let receiver = daemon
        .browse(SERVICE_TYPE)
        .map_err(|_| AppError::storage("No se pudo iniciar la búsqueda"))?;
    let deadline = Instant::now() + std::time::Duration::from_secs(3);
    let mut found = HashMap::new();
    while Instant::now() < deadline {
        if let Ok(ServiceEvent::ServiceResolved(info)) =
            receiver.recv_timeout(std::time::Duration::from_millis(200))
        {
            let Some(address) = info
                .get_addresses_v4()
                .into_iter()
                .find(|ip| local_ip(&ip.to_string()).is_ok())
            else {
                continue;
            };
            let get = |key: &str| info.get_property_val_str(key).unwrap_or("").to_string();
            let cert = (0..24)
                .map(|i| get(&format!("cert{i}")))
                .collect::<String>();
            let Ok(cert) = STANDARD.decode(cert) else {
                continue;
            };
            let Ok(certificate) = String::from_utf8(cert) else {
                continue;
            };
            let host = HostIdentity {
                station_id: get("station_id"),
                address: address.to_string(),
                port: info.get_port(),
                certificate,
                fingerprint: get("fingerprint"),
                name: get("name"),
            };
            if validate_identity(&host).is_ok() {
                found.insert(host.station_id.clone(), host);
            }
        }
    }
    let _ = daemon.shutdown();
    Ok(found.into_values().collect())
}
