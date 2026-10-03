use super::*;
use base64::{engine::general_purpose::STANDARD, Engine};
use mdns_sd::{DaemonEvent, ServiceDaemon, ServiceEvent, ServiceInfo};

#[derive(Clone, Serialize)]
pub struct NearbyStation {
    pub station_id: String,
    pub name: String,
    pub address: String,
}

pub fn client_addresses() -> Vec<String> {
    let mut addresses = if_addrs::get_if_addrs()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|interface| match interface.ip() {
            std::net::IpAddr::V4(ip)
                if !ip.is_loopback() && local_ip(&ip.to_string()).is_ok() =>
            {
                Some(ip.to_string())
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    addresses.sort();
    addresses.dedup();
    addresses
}

pub fn advertise_client(
    station_id: &str,
    name: &str,
    addresses: &[String],
) -> AppResult<ServiceDaemon> {
    let daemon = ServiceDaemon::new()
        .map_err(|_| AppError::storage("No se pudo anunciar este puesto en la red"))?;
    for address in addresses {
        let instance = format!("{}-{}", station_id, address.replace('.', "-"));
        let service = ServiceInfo::new(
            CLIENT_SERVICE_TYPE,
            &instance,
            &format!("nightdesk-station-{}.local.", station_id),
            address.as_str(),
            9,
            HashMap::from([
                ("station_id".to_string(), station_id.to_string()),
                ("name".to_string(), name.to_string()),
            ]),
        )
        .map_err(|_| AppError::storage("No se pudo crear el anuncio de este puesto"))?;
        daemon
            .register(service)
            .map_err(|_| AppError::storage("No se pudo publicar este puesto en la red"))?;
    }
    Ok(daemon)
}

pub fn discover_clients() -> AppResult<Vec<NearbyStation>> {
    let daemon = ServiceDaemon::new()
        .map_err(|_| AppError::storage("No se pudo buscar puestos adicionales"))?;
    let receiver = daemon
        .browse(CLIENT_SERVICE_TYPE)
        .map_err(|_| AppError::storage("No se pudo iniciar la búsqueda de puestos"))?;
    let deadline = Instant::now() + std::time::Duration::from_secs(3);
    let mut found = HashMap::new();
    while Instant::now() < deadline {
        if let Ok(ServiceEvent::ServiceResolved(info)) =
            receiver.recv_timeout(std::time::Duration::from_millis(200))
        {
            let station_id = info.get_property_val_str("station_id").unwrap_or("");
            if uuid::Uuid::parse_str(station_id).is_err() {
                continue;
            }
            let Some(address) = info
                .get_addresses_v4()
                .into_iter()
                .find(|ip| local_ip(&ip.to_string()).is_ok())
            else {
                continue;
            };
            found.insert(
                station_id.to_string(),
                NearbyStation {
                    station_id: station_id.to_string(),
                    name: info.get_property_val_str("name").unwrap_or("Recepción adicional").to_string(),
                    address: address.to_string(),
                },
            );
        }
    }
    let _ = daemon.shutdown();
    Ok(found.into_values().collect())
}

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
    let monitor = daemon
        .monitor()
        .map_err(|e| AppError::storage(format!("No se pudo observar Bonjour: {e}")))?;
    daemon
        .register(service)
        .map_err(|e| AppError::storage(format!("No se pudo anunciar recepción: {e}")))?;
    let deadline = Instant::now() + std::time::Duration::from_secs(3);
    while Instant::now() < deadline {
        match monitor.recv_timeout(std::time::Duration::from_millis(200)) {
            Ok(DaemonEvent::Announce(_, _)) => return Ok(daemon),
            Ok(DaemonEvent::Error(error)) => {
                let _ = daemon.shutdown();
                return Err(AppError::storage(format!(
                    "Bonjour no pudo publicar la recepción: {error}"
                )));
            }
            _ => {}
        }
    }
    let _ = daemon.shutdown();
    Err(AppError::storage(
        "Bonjour no confirmó el anuncio de la recepción",
    ))
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
